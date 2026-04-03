use std::env;
use std::os::unix::net::UnixStream;

use greetd_ipc::codec::SyncCodec;
use greetd_ipc::{Request, Response};

use crate::error::GreeterError;

// -- Connection trait ---------------------------------------------------------

/// Abstraction over the greetd socket for testability.
pub trait GreetdConnection {
    fn send(&mut self, req: Request) -> Result<(), GreeterError>;
    fn recv(&mut self) -> Result<Response, GreeterError>;
}

/// Real connection over a Unix socket.
struct SocketConnection(UnixStream);

impl GreetdConnection for SocketConnection {
    fn send(&mut self, req: Request) -> Result<(), GreeterError> {
        req.write_to(&mut self.0)?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Response, GreeterError> {
        Ok(Response::read_from(&mut self.0)?)
    }
}

pub fn connect() -> Result<impl GreetdConnection, GreeterError> {
    let sock_path =
        env::var("GREETD_SOCK").map_err(|_| GreeterError::Ipc("GREETD_SOCK not set".into()))?;
    let stream = UnixStream::connect(&sock_path)
        .map_err(|e| GreeterError::Ipc(format!("connect: {e}")))?;
    Ok(SocketConnection(stream))
}

// -- Auth flow ----------------------------------------------------------------

/// Outcome of a full authentication + session-start attempt.
#[derive(Debug, PartialEq)]
pub enum AuthOutcome {
    SessionStarted,
    AuthFailed(String),
    SessionError(String),
}

/// Run the three-step greetd IPC flow against any connection.
pub fn authenticate(
    conn: &mut dyn GreetdConnection,
    username: &str,
    password: &str,
    session_cmd: &[String],
) -> Result<AuthOutcome, GreeterError> {
    // Step 1: create_session
    conn.send(Request::CreateSession {
        username: username.to_string(),
    })?;

    match conn.recv()? {
        Response::AuthMessage { .. } => {}
        Response::Error { description, .. } => {
            return Ok(AuthOutcome::AuthFailed(description));
        }
        Response::Success => {
            return start_session(conn, session_cmd);
        }
    }

    // Step 2: post password
    conn.send(Request::PostAuthMessageResponse {
        response: Some(password.to_string()),
    })?;

    match conn.recv()? {
        Response::Success => {}
        Response::Error { description, .. } => {
            return Ok(AuthOutcome::AuthFailed(description));
        }
        Response::AuthMessage { .. } => {
            return Ok(AuthOutcome::AuthFailed("unexpected auth message".into()));
        }
    }

    // Step 3: start session
    start_session(conn, session_cmd)
}

fn start_session(
    conn: &mut dyn GreetdConnection,
    cmd: &[String],
) -> Result<AuthOutcome, GreeterError> {
    conn.send(Request::StartSession {
        cmd: cmd.to_vec(),
        env: Vec::new(),
    })?;

    match conn.recv()? {
        Response::Success => Ok(AuthOutcome::SessionStarted),
        Response::Error { description, .. } => Ok(AuthOutcome::SessionError(description)),
        Response::AuthMessage { .. } => Ok(AuthOutcome::SessionError("unexpected response".into())),
    }
}

// -- Tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use greetd_ipc::{AuthMessageType, ErrorType};
    use std::collections::VecDeque;

    fn test_cmd() -> Vec<String> {
        vec!["Hyprland".into()]
    }

    struct MockConnection {
        expected_requests: VecDeque<Request>,
        responses: VecDeque<Response>,
    }

    impl MockConnection {
        fn new(responses: Vec<Response>) -> Self {
            Self {
                expected_requests: VecDeque::new(),
                responses: VecDeque::from(responses),
            }
        }

        fn with_expectations(expected: Vec<Request>, responses: Vec<Response>) -> Self {
            Self {
                expected_requests: VecDeque::from(expected),
                responses: VecDeque::from(responses),
            }
        }
    }

    impl GreetdConnection for MockConnection {
        fn send(&mut self, req: Request) -> Result<(), GreeterError> {
            if let Some(expected) = self.expected_requests.pop_front() {
                let req_json = serde_json::to_string(&req).unwrap();
                let exp_json = serde_json::to_string(&expected).unwrap();
                assert_eq!(req_json, exp_json, "unexpected request sent to greetd");
            }
            Ok(())
        }

        fn recv(&mut self) -> Result<Response, GreeterError> {
            self.responses
                .pop_front()
                .ok_or_else(|| GreeterError::Ipc("no more mock responses".into()))
        }
    }

    fn auth_prompt() -> Response {
        Response::AuthMessage {
            auth_message_type: AuthMessageType::Secret,
            auth_message: "Password:".into(),
        }
    }

    fn auth_error(msg: &str) -> Response {
        Response::Error {
            error_type: ErrorType::AuthError,
            description: msg.into(),
        }
    }

    fn session_error(msg: &str) -> Response {
        Response::Error {
            error_type: ErrorType::Error,
            description: msg.into(),
        }
    }

    #[test]
    fn success_flow() {
        let cmd = test_cmd();
        let mut conn = MockConnection::with_expectations(
            vec![
                Request::CreateSession { username: "stark".into() },
                Request::PostAuthMessageResponse { response: Some("secret".into()) },
                Request::StartSession { cmd: cmd.clone(), env: Vec::new() },
            ],
            vec![auth_prompt(), Response::Success, Response::Success],
        );

        let result = authenticate(&mut conn, "stark", "secret", &cmd).unwrap();
        assert_eq!(result, AuthOutcome::SessionStarted);
    }

    #[test]
    fn wrong_password() {
        let mut conn = MockConnection::new(vec![auth_prompt(), auth_error("Authentication failed")]);
        let result = authenticate(&mut conn, "stark", "wrong", &test_cmd()).unwrap();
        assert_eq!(result, AuthOutcome::AuthFailed("Authentication failed".into()));
    }

    #[test]
    fn unknown_user() {
        let mut conn = MockConnection::new(vec![auth_error("user not found")]);
        let result = authenticate(&mut conn, "nobody", "pass", &test_cmd()).unwrap();
        assert_eq!(result, AuthOutcome::AuthFailed("user not found".into()));
    }

    #[test]
    fn no_auth_needed() {
        let cmd = test_cmd();
        let mut conn = MockConnection::with_expectations(
            vec![
                Request::CreateSession { username: "autologin".into() },
                Request::StartSession { cmd: cmd.clone(), env: Vec::new() },
            ],
            vec![Response::Success, Response::Success],
        );
        let result = authenticate(&mut conn, "autologin", "", &cmd).unwrap();
        assert_eq!(result, AuthOutcome::SessionStarted);
    }

    #[test]
    fn session_start_fails() {
        let mut conn = MockConnection::new(vec![
            auth_prompt(), Response::Success, session_error("launch failed"),
        ]);
        let result = authenticate(&mut conn, "stark", "pass", &test_cmd()).unwrap();
        assert_eq!(result, AuthOutcome::SessionError("launch failed".into()));
    }

    #[test]
    fn unexpected_second_auth_message() {
        let mut conn = MockConnection::new(vec![
            auth_prompt(),
            Response::AuthMessage {
                auth_message_type: AuthMessageType::Secret,
                auth_message: "Second factor:".into(),
            },
        ]);
        let result = authenticate(&mut conn, "stark", "pass", &test_cmd()).unwrap();
        assert_eq!(result, AuthOutcome::AuthFailed("unexpected auth message".into()));
    }

    #[test]
    fn connection_error_propagates() {
        struct FailingConnection;
        impl GreetdConnection for FailingConnection {
            fn send(&mut self, _: Request) -> Result<(), GreeterError> {
                Err(GreeterError::Ipc("connection refused".into()))
            }
            fn recv(&mut self) -> Result<Response, GreeterError> {
                Err(GreeterError::Ipc("not connected".into()))
            }
        }
        let result = authenticate(&mut FailingConnection, "stark", "pass", &test_cmd());
        assert!(result.is_err());
    }

    #[test]
    fn custom_session_cmd_sent() {
        let cmd = vec!["uwsm".into(), "start".into(), "hyprland-uwsm.desktop".into()];
        let mut conn = MockConnection::with_expectations(
            vec![
                Request::CreateSession { username: "stark".into() },
                Request::PostAuthMessageResponse { response: Some("pass".into()) },
                Request::StartSession { cmd: cmd.clone(), env: Vec::new() },
            ],
            vec![auth_prompt(), Response::Success, Response::Success],
        );
        let result = authenticate(&mut conn, "stark", "pass", &cmd).unwrap();
        assert_eq!(result, AuthOutcome::SessionStarted);
    }
}
