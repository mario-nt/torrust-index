use actix_web::{web, Responder, HttpResponse, HttpRequest};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use serde::{Deserialize, Serialize};
use jsonwebtoken::{DecodingKey, decode, Validation, Algorithm};
use rand_core::OsRng;

use crate::errors::{ServiceResult, ServiceError};
use crate::common::WebAppData;
use crate::config::{EmailOnSignup, InvitatiOnOnSignup};
use crate::models::response::OkResponse;
use crate::models::response::TokenResponse;
use crate::mailer::VerifyClaims;
use crate::utils::regex::validate_email_address;
use crate::utils::time::current_time;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user")
            .service(web::resource("/register")
                .route(web::post().to(register)))
            .service(web::resource("/login")
                .route(web::post().to(login)))
            .service(web::resource("/ban/{user}")
                .route(web::delete().to(ban_user)))
            .service(web::resource("/token/verify")
                .route(web::post().to(verify_token)))
            .service(web::resource("/token/renew")
                .route(web::post().to(renew_token)))
            .service(web::resource("/email/verify/{token}")
                .route(web::get().to(verify_email)))
    );
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Register {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
    pub confirm_password: String,
    pub invitation: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Login {
    pub login: String,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Token {
    pub token: String,
}

pub async fn register(req: HttpRequest, mut payload: web::Json<Register>, app_data: WebAppData) -> ServiceResult<impl Responder> {
    let settings = app_data.cfg.settings.read().await;

    match settings.auth.invitation_on_signup {
        InvitatiOnOnSignup::Required => {
            if payload.invitation.is_none() { return Err(ServiceError::InvitationMissing) }
        }
        InvitatiOnOnSignup::None => {
            payload.invitation = None
        }
        _ => {}
    }

    // TODO: Verify invitation.

    match settings.auth.email_on_signup {
        EmailOnSignup::Required => {
            if payload.email.is_none() { return Err(ServiceError::EmailMissing) }
        }
        EmailOnSignup::None => {
            payload.email = None
        }
        _ => {}
    }

    if let Some(email) = &payload.email {
        // check if email address is valid
        if !validate_email_address(email) {
            return Err(ServiceError::EmailInvalid)
        }
    }

    if payload.password != payload.confirm_password {
        return Err(ServiceError::PasswordsDontMatch)
    }

    let password_length = payload.password.len();

    if password_length <= settings.auth.min_password_length {
        return Err(ServiceError::PasswordTooShort)
    }

    if password_length >= settings.auth.max_password_length {
        return Err(ServiceError::PasswordTooLong)
    }

    let salt = SaltString::generate(&mut OsRng);

    // Argon2 with default params (Argon2id v19)
    let argon2 = Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2.hash_password(payload.password.as_bytes(), &salt)?.to_string();

    if payload.username.contains('@') {
        return Err(ServiceError::UsernameInvalid)
    }

    let email = payload.email.as_ref().unwrap_or(&"".to_string()).to_string();

    let user_id = app_data.database.insert_user_and_get_id(&payload.username, &email, &password_hash).await?;

    // if this is the first created account, give administrator rights
    if user_id == 1 {
        let _ = app_data.database.grant_admin_role(user_id).await;
    }

    let conn_info = req.connection_info();

    if settings.mail.email_verification_enabled && payload.email.is_some() {
        let mail_res = app_data.mailer.send_verification_mail(
            payload.email.as_ref().unwrap(),
            &payload.username,
            user_id,
            format!("{}://{}", conn_info.scheme(), conn_info.host()).as_str()
        )
            .await;

        if mail_res.is_err() {
            let _ = app_data.database.delete_user(user_id).await;
            return Err(ServiceError::FailedToSendVerificationEmail)
        }
    }

    Ok(HttpResponse::Ok())
}

pub async fn login(payload: web::Json<Login>, app_data: WebAppData) -> ServiceResult<impl Responder> {
    // get the user profile from database
    let user_profile = app_data.database.get_user_profile_from_username(&payload.login)
        .await
        .map_err(|_| ServiceError::WrongPasswordOrUsername)?;

    // should not be able to fail if user_profile succeeded
    let user_authentication = app_data.database.get_user_authentication_from_id(user_profile.user_id)
        .await
        .map_err(|_| ServiceError::InternalServerError)?;

    // wrap string of the hashed password into a PasswordHash struct for verification
    let parsed_hash = PasswordHash::new(&user_authentication.password_hash)?;

    // verify if the user supplied and the database supplied passwords match
    if Argon2::default().verify_password(payload.password.as_bytes(), &parsed_hash).is_err() {
        return Err(ServiceError::WrongPasswordOrUsername)
    }

    let settings = app_data.cfg.settings.read().await;

    // fail login if email verification is required and this email is not verified
    if settings.mail.email_verification_enabled && !user_profile.email_verified {
        return Err(ServiceError::EmailNotVerified)
    }

    // drop read lock on settings
    drop(settings);

    let user_compact = app_data.database.get_user_compact_from_id(user_profile.user_id).await?;

    // sign jwt with compact user details as payload
    let token = app_data.auth.sign_jwt(user_compact.clone()).await;


    Ok(HttpResponse::Ok().json(OkResponse {
        data: TokenResponse {
            token,
            username: user_compact.username,
            admin: user_compact.administrator
        }
    }))
}

pub async fn verify_token(payload: web::Json<Token>, app_data: WebAppData) -> ServiceResult<impl Responder> {
    // verify if token is valid
    let _claims = app_data.auth.verify_jwt(&payload.token).await?;

    Ok(HttpResponse::Ok().json(OkResponse {
        data: format!("Token is valid.")
    }))
}

pub async fn renew_token(payload: web::Json<Token>, app_data: WebAppData) -> ServiceResult<impl Responder> {
    // verify if token is valid
    let claims = app_data.auth.verify_jwt(&payload.token).await?;

    let user_compact = app_data.database.get_user_compact_from_id(claims.user.user_id).await?;

    const ONE_WEEK_IN_SECONDS: u64 = 604_800;

    // renew token if it is valid for less than one week
    let token = match claims.exp - current_time() {
        x if x < ONE_WEEK_IN_SECONDS => app_data.auth.sign_jwt(user_compact.clone()).await,
        _ => payload.token.clone()
    };

    Ok(HttpResponse::Ok().json(OkResponse {
        data: TokenResponse {
            token,
            username: user_compact.username,
            admin: user_compact.administrator
        }
    }))
}

pub async fn verify_email(req: HttpRequest, app_data: WebAppData) -> String {
    let settings = app_data.cfg.settings.read().await;
    let token = req.match_info().get("token").unwrap();

    let token_data = match decode::<VerifyClaims>(
        token,
        &DecodingKey::from_secret(settings.auth.secret_key.as_bytes()),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => {
            if !token_data.claims.iss.eq("email-verification") {
                return ServiceError::TokenInvalid.to_string()
            }

            token_data.claims
        },
        Err(_) => return ServiceError::TokenInvalid.to_string()
    };

    drop(settings);

    if app_data.database.verify_email(token_data.sub).await.is_err() {
        return ServiceError::InternalServerError.to_string()
    };

    String::from("Email verified, you can close this page.")
}

// TODO: add reason and date_expiry parameters to request
pub async fn ban_user(req: HttpRequest, app_data: WebAppData) -> ServiceResult<impl Responder> {
    let user = app_data.auth.get_user_compact_from_request(&req).await?;

    // check if user is administrator
    if !user.administrator { return Err(ServiceError::Unauthorized) }

    let to_be_banned_username = req.match_info().get("user").unwrap();

    let user_profile = app_data.database.get_user_profile_from_username(to_be_banned_username).await?;

    let reason = "no reason".to_string();

    // user will be banned until the year 9999
    let date_expiry = chrono::NaiveDateTime::parse_from_str("9999-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").expect("Could not parse date from 9999-01-01 00:00:00.");

    let _ = app_data.database.ban_user(user_profile.user_id, &reason, date_expiry).await?;

    Ok(HttpResponse::Ok().json(OkResponse {
        data: format!("Banned user: {}", to_be_banned_username)
    }))
}
