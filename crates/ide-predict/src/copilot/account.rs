//! The GitHub account Copilot works for, as `checkStatus` and `signIn` report it.

use std::time::Duration;

use serde_json::Value;

use crate::texts;

/// Where Copilot stands with the user's GitHub account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Account {
    /// The server is starting and has not said yet.
    Starting,
    /// Signed in with a subscription; proposals come.
    SignedIn {
        /// The GitHub login.
        user: String,
    },
    /// Nobody is signed in; [`super::Copilot::sign_in`] starts the device flow.
    NeedsSignIn,
    /// Signed in, but the account has no Copilot subscription, not even the free one.
    NoSubscription {
        /// The GitHub login.
        user: String,
    },
    /// The server cannot work; the text says why, in the interface language.
    Unavailable(String),
}

/// What [`super::Copilot::sign_in`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignIn {
    /// Someone is signed in already.
    Already {
        /// The GitHub login.
        user: String,
    },
    /// The user enters `user_code` at `verification_uri`; [`super::Copilot::finish_sign_in`] waits for it.
    Device(DeviceCode),
}

/// The code of GitHub's device flow, which the user types on GitHub's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    /// The code, such as `ABCD-EFGH`.
    pub user_code: String,
    /// The page that asks for it.
    pub verification_uri: String,
    /// How long the code is good for.
    pub expires_in: Duration,
}

/// GitHub's page for device codes, where the server sends the user when it does not name one.
const DEVICE_PAGE: &str = "https://github.com/login/device";

/// How long a device code lasts when the server does not say; GitHub's codes last 15 minutes.
const DEVICE_LIFETIME: Duration = Duration::from_secs(15 * 60);

/// The account an answer to `checkStatus` reports.
pub(crate) fn status(answer: &Value) -> Account {
    let user = || {
        answer
            .get("user")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match answer.get("status").and_then(Value::as_str) {
        Some("OK" | "MaybeOK" | "AlreadySignedIn") => Account::SignedIn { user: user() },
        Some("NotAuthorized") => Account::NoSubscription { user: user() },
        Some("NotSignedIn") => Account::NeedsSignIn,
        Some(other) => Account::Unavailable(texts::failed("Copilot", other)),
        None => Account::Unavailable(texts::failed("Copilot", &answer.to_string())),
    }
}

/// What an answer to `signIn` asks of the user, and the command that finishes the flow.
pub(crate) fn sign_in(answer: &Value) -> Option<(SignIn, Option<Value>)> {
    if answer.get("status").and_then(Value::as_str) == Some("AlreadySignedIn") {
        let user = answer.get("user").and_then(Value::as_str).unwrap_or_default();
        return Some((SignIn::Already { user: user.to_string() }, None));
    }
    let user_code = answer.get("userCode").and_then(Value::as_str)?.to_string();
    let verification_uri = answer
        .get("verificationUri")
        .and_then(Value::as_str)
        .unwrap_or(DEVICE_PAGE)
        .to_string();
    let expires_in = answer
        .get("expiresIn")
        .and_then(Value::as_u64)
        .map_or(DEVICE_LIFETIME, Duration::from_secs);
    let command = answer.get("command").filter(|c| c.is_object()).cloned();
    let code = DeviceCode {
        user_code,
        verification_uri,
        expires_in,
    };
    Some((SignIn::Device(code), command))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn check_status_answers_become_accounts() {
        assert_eq!(
            status(&json!({"status": "OK", "user": "octocat"})),
            Account::SignedIn { user: "octocat".into() }
        );
        assert_eq!(
            status(&json!({"status": "NotAuthorized", "user": "octocat"})),
            Account::NoSubscription { user: "octocat".into() }
        );
        assert_eq!(status(&json!({"status": "NotSignedIn"})), Account::NeedsSignIn);
        assert!(matches!(
            status(&json!({"status": "FailedToGetToken"})),
            Account::Unavailable(_)
        ));
        assert!(matches!(status(&json!(null)), Account::Unavailable(_)));
    }

    #[test]
    fn a_sign_in_answer_gives_the_device_code_or_the_signed_in_user() {
        let device = json!({
            "status": "PromptUserDeviceFlow",
            "userCode": "ABCD-EFGH",
            "verificationUri": "https://github.com/login/device",
            "expiresIn": 899,
            "interval": 5,
            "command": {"command": "github.copilot.finishDeviceFlow", "arguments": [], "title": "Sign in"},
        });
        let (sign_in, command) = super::sign_in(&device).unwrap();
        assert_eq!(
            sign_in,
            SignIn::Device(DeviceCode {
                user_code: "ABCD-EFGH".into(),
                verification_uri: DEVICE_PAGE.into(),
                expires_in: Duration::from_secs(899),
            })
        );
        assert_eq!(command.unwrap()["command"], "github.copilot.finishDeviceFlow");
        let bare = json!({"userCode": "WXYZ-1234", "command": {"command": "github.copilot.finishDeviceFlow"}});
        let Some((SignIn::Device(code), _)) = super::sign_in(&bare) else {
            panic!()
        };
        assert_eq!(
            (code.verification_uri.as_str(), code.expires_in),
            (DEVICE_PAGE, DEVICE_LIFETIME)
        );
        assert_eq!(
            super::sign_in(&json!({"status": "AlreadySignedIn", "user": "octocat"})).map(|s| s.0),
            Some(SignIn::Already { user: "octocat".into() })
        );
        assert_eq!(super::sign_in(&json!({"status": "Weird"})), None);
    }
}
