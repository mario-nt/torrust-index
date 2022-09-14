use std::borrow::Cow;
use derive_more::{Display, Error};
use actix_web::{ResponseError, HttpResponse, HttpResponseBuilder};
use actix_web::http::{header, StatusCode};
use serde::{Deserialize, Serialize};
use std::error;
use crate::databases::database::DatabaseError;

pub type ServiceResult<V> = Result<V, ServiceError>;

#[derive(Debug, Display, PartialEq, Error)]
#[allow(dead_code)]
pub enum ServiceError {
    #[display(fmt = "internal server error")]
    InternalServerError,

    #[display(
    fmt = "This server is is closed for registration. Contact admin if this is unexpected"
    )]
    ClosedForRegistration,

    #[display(fmt = "Email is required")] //405j
    EmailMissing,
    #[display(fmt = "Please enter a valid email address")] //405j
    EmailInvalid,

    #[display(fmt = "The value you entered for URL is not a URL")] //405j
    NotAUrl,

    #[display(fmt = "Invalid username/email or password")]
    WrongPasswordOrUsername,
    #[display(fmt = "Username not found")]
    UsernameNotFound,
    #[display(fmt = "User not found")]
    UserNotFound,

    #[display(fmt = "Account not found")]
    AccountNotFound,

    /// when the value passed contains profanity
    #[display(fmt = "Can't allow profanity in usernames")]
    ProfanityError,
    /// when the value passed contains blacklisted words
    /// see [blacklist](https://github.com/shuttlecraft/The-Big-Username-Blacklist)
    #[display(fmt = "Username contains blacklisted words")]
    BlacklistError,
    /// when the value passed contains characters not present
    /// in [UsernameCaseMapped](https://tools.ietf.org/html/rfc8265#page-7)
    /// profile
    #[display(fmt = "username_case_mapped violation")]
    UsernameCaseMappedError,

    #[display(fmt = "Password too short")]
    PasswordTooShort,
    #[display(fmt = "Username too long")]
    PasswordTooLong,
    #[display(fmt = "Passwords don't match")]
    PasswordsDontMatch,

    /// when the a username is already taken
    #[display(fmt = "Username not available")]
    UsernameTaken,

    #[display(fmt = "Username contains illegal characters")]
    UsernameInvalid,

    /// email is already taken
    #[display(fmt = "Email not available")]
    EmailTaken,

    #[display(fmt = "Please verify your email before logging in")]
    EmailNotVerified,

    /// when the a token name is already taken
    /// token not found
    #[display(fmt = "Token not found. Please sign in.")]
    TokenNotFound,

    /// token expired
    #[display(fmt = "Token expired. Please sign in again.")]
    TokenExpired,

    #[display(fmt = "Token invalid.")]
    /// token invalid
    TokenInvalid,

    #[display(fmt = "Torrent not found.")]
    TorrentNotFound,

    #[display(fmt = "Uploaded torrent is not valid.")]
    InvalidTorrentFile,

    #[display(fmt = "Uploaded torrent has an invalid pieces key.")]
    InvalidTorrentPiecesLength,

    #[display(fmt = "Only .torrent files can be uploaded.")]
    InvalidFileType,

    #[display(fmt = "Bad request.")]
    BadRequest,

    #[display(fmt = "Selected category does not exist.")]
    InvalidCategory,

    #[display(fmt = "Unauthorized action.")]
    Unauthorized,

    #[display(fmt = "This torrent already exists in our database.")]
    InfoHashAlreadyExists,

    #[display(fmt = "This torrent title has already been used.")]
    TorrentTitleAlreadyExists,

    #[display(fmt = "Sorry, we have an error with our tracker connection.")]
    TrackerOffline,

    #[display(fmt = "Failed to send verification email.")]
    FailedToSendVerificationEmail,

    #[display(fmt = "Category already exists..")]
    CategoryExists,

    #[display(fmt = "Invitation is required.")]
    InvitationMissing,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorToResponse {
    pub error: String,
}

impl ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match self {
            ServiceError::ClosedForRegistration => StatusCode::FORBIDDEN,
            ServiceError::EmailInvalid => StatusCode::BAD_REQUEST,
            ServiceError::NotAUrl => StatusCode::BAD_REQUEST,
            ServiceError::WrongPasswordOrUsername => StatusCode::FORBIDDEN,
            ServiceError::UsernameNotFound => StatusCode::NOT_FOUND,
            ServiceError::UserNotFound => StatusCode::NOT_FOUND,
            ServiceError::AccountNotFound => StatusCode::NOT_FOUND,

            ServiceError::ProfanityError => StatusCode::BAD_REQUEST,
            ServiceError::BlacklistError => StatusCode::BAD_REQUEST,
            ServiceError::UsernameCaseMappedError => StatusCode::BAD_REQUEST,

            ServiceError::PasswordTooShort => StatusCode::BAD_REQUEST,
            ServiceError::PasswordTooLong => StatusCode::BAD_REQUEST,
            ServiceError::PasswordsDontMatch => StatusCode::BAD_REQUEST,

            ServiceError::UsernameTaken => StatusCode::BAD_REQUEST,
            ServiceError::UsernameInvalid => StatusCode::BAD_REQUEST,
            ServiceError::EmailTaken => StatusCode::BAD_REQUEST,
            ServiceError::EmailNotVerified => StatusCode::FORBIDDEN,

            ServiceError::TokenNotFound => StatusCode::UNAUTHORIZED,
            ServiceError::TokenExpired => StatusCode::UNAUTHORIZED,
            ServiceError::TokenInvalid => StatusCode::UNAUTHORIZED,

            ServiceError::TorrentNotFound => StatusCode::BAD_REQUEST,

            ServiceError::InvalidTorrentFile => StatusCode::BAD_REQUEST,
            ServiceError::InvalidTorrentPiecesLength => StatusCode::BAD_REQUEST,
            ServiceError::InvalidFileType => StatusCode::BAD_REQUEST,

            ServiceError::BadRequest => StatusCode::BAD_REQUEST,

            ServiceError::InvalidCategory => StatusCode::BAD_REQUEST,

            ServiceError::Unauthorized => StatusCode::FORBIDDEN,

            ServiceError::InfoHashAlreadyExists => StatusCode::BAD_REQUEST,

            ServiceError::TorrentTitleAlreadyExists => StatusCode::BAD_REQUEST,

            ServiceError::TrackerOffline => StatusCode::INTERNAL_SERVER_ERROR,

            ServiceError::CategoryExists => StatusCode::BAD_REQUEST,

            ServiceError::InvitationMissing => StatusCode::BAD_REQUEST,

            _ => StatusCode::INTERNAL_SERVER_ERROR
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(self.status_code())
            .append_header((header::CONTENT_TYPE, "application/json; charset=UTF-8"))
            .body(
                serde_json::to_string(&ErrorToResponse {
                    error: self.to_string(),
                })
                    .unwrap(),
            )
            .into()
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(e: sqlx::Error) -> Self {
        eprintln!("{:?}", e);

        if let Some(err) = e.as_database_error() {
            return if err.code() == Some(Cow::from("2067")) {
                if err.message().contains("torrust_torrents.info_hash") {
                    ServiceError::InfoHashAlreadyExists
                } else {
                    ServiceError::InternalServerError
                }
            } else {
                ServiceError::TorrentNotFound
            }
        }

        ServiceError::InternalServerError
    }
}

impl From<DatabaseError> for ServiceError {
    fn from(e: DatabaseError) -> Self {
        match e {
            DatabaseError::Error => ServiceError::InternalServerError,
            DatabaseError::UsernameTaken => ServiceError::UsernameTaken,
            DatabaseError::EmailTaken => ServiceError::EmailTaken,
            DatabaseError::UserNotFound => ServiceError::UserNotFound,
            DatabaseError::CategoryAlreadyExists => ServiceError::CategoryExists,
            DatabaseError::CategoryNotFound => ServiceError::InvalidCategory,
            DatabaseError::TorrentNotFound => ServiceError::TorrentNotFound,
            DatabaseError::TorrentAlreadyExists => ServiceError::InfoHashAlreadyExists,
            DatabaseError::TorrentTitleAlreadyExists => ServiceError::TorrentTitleAlreadyExists,
            DatabaseError::UnrecognizedDatabaseDriver => ServiceError::InternalServerError,
        }
    }
}

impl From<argon2::password_hash::Error> for ServiceError {
    fn from(e: argon2::password_hash::Error) -> Self {
        eprintln!("{}", e);
        ServiceError::InternalServerError
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(e: std::io::Error) -> Self {
        eprintln!("{}", e);
        ServiceError::InternalServerError
    }
}

impl From<Box<dyn error::Error>> for ServiceError {
    fn from(e: Box<dyn error::Error>) -> Self {
        eprintln!("{}", e);
        ServiceError::InternalServerError
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(e: serde_json::Error) -> Self {
        eprintln!("{}", e);
        ServiceError::InternalServerError
    }
}
