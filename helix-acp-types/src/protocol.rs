//! Protocol trait definitions for ACP requests and notifications

use super::*;

/// Trait for ACP request types
pub trait Request {
    /// The method name for this request
    const METHOD: &'static str;
    /// The parameters type
    type Params: serde::Serialize + for<'de> serde::Deserialize<'de>;
    /// The result type
    type Result: serde::Serialize + for<'de> serde::Deserialize<'de>;
}

/// Trait for ACP notification types
pub trait Notification {
    /// The method name for this notification
    const METHOD: &'static str;
    /// The parameters type
    type Params: serde::Serialize + for<'de> serde::Deserialize<'de>;
}

/// Namespace for request types (client -> agent)
pub mod requests {
    use super::*;

    /// Initialize request
    pub struct Initialize;
    impl Request for Initialize {
        const METHOD: &'static str = "session/initialize";
        type Params = super::InitializeRequest;
        type Result = super::InitializeResponse;
    }

    /// New session request
    pub struct NewSession;
    impl Request for NewSession {
        const METHOD: &'static str = "session/new";
        type Params = super::NewSessionRequest;
        type Result = super::NewSessionResponse;
    }

    /// Prompt request
    pub struct Prompt;
    impl Request for Prompt {
        const METHOD: &'static str = "session/prompt";
        type Params = super::PromptRequest;
        type Result = super::PromptResponse;
    }

    /// Cancel session request
    pub struct CancelSession;
    impl Request for CancelSession {
        const METHOD: &'static str = "session/cancel";
        type Params = super::CancelSessionRequest;
        type Result = super::CancelSessionResponse;
    }

    // Client-side requests (agent -> client)

    /// Request permission from user
    pub struct RequestPermission;
    impl Request for RequestPermission {
        const METHOD: &'static str = "client/requestPermission";
        type Params = super::RequestPermissionRequest;
        type Result = super::RequestPermissionResponse;
    }

    /// Read text file
    pub struct ReadTextFile;
    impl Request for ReadTextFile {
        const METHOD: &'static str = "client/readTextFile";
        type Params = super::ReadTextFileRequest;
        type Result = super::ReadTextFileResponse;
    }

    /// Write text file
    pub struct WriteTextFile;
    impl Request for WriteTextFile {
        const METHOD: &'static str = "client/writeTextFile";
        type Params = super::WriteTextFileRequest;
        type Result = super::WriteTextFileResponse;
    }

    /// Create terminal
    pub struct CreateTerminal;
    impl Request for CreateTerminal {
        const METHOD: &'static str = "client/createTerminal";
        type Params = super::CreateTerminalRequest;
        type Result = super::CreateTerminalResponse;
    }

    /// Terminal output
    pub struct TerminalOutput;
    impl Request for TerminalOutput {
        const METHOD: &'static str = "client/terminalOutput";
        type Params = super::TerminalOutputRequest;
        type Result = super::TerminalOutputResponse;
    }

    /// Release terminal
    pub struct ReleaseTerminal;
    impl Request for ReleaseTerminal {
        const METHOD: &'static str = "client/releaseTerminal";
        type Params = super::ReleaseTerminalRequest;
        type Result = super::ReleaseTerminalResponse;
    }

    /// Wait for terminal exit
    pub struct WaitForTerminalExit;
    impl Request for WaitForTerminalExit {
        const METHOD: &'static str = "client/waitForTerminalExit";
        type Params = super::WaitForTerminalExitRequest;
        type Result = super::WaitForTerminalExitResponse;
    }

    /// Kill terminal command
    pub struct KillTerminalCommand;
    impl Request for KillTerminalCommand {
        const METHOD: &'static str = "client/killTerminalCommand";
        type Params = super::KillTerminalCommandRequest;
        type Result = super::KillTerminalCommandResponse;
    }

    /// Extension method
    pub struct ExtMethod;
    impl Request for ExtMethod {
        const METHOD: &'static str = "client/ext";
        type Params = super::ExtRequest;
        type Result = super::ExtResponse;
    }
}

/// Namespace for notification types
pub mod notifications {
    use super::*;

    /// Session notification (agent -> client)
    pub struct SessionNotification;
    impl Notification for SessionNotification {
        const METHOD: &'static str = "session/update";
        type Params = super::SessionNotification;
    }

    /// Exit notification (client -> agent)
    pub struct Exit;
    impl Notification for Exit {
        const METHOD: &'static str = "session/exit";
        type Params = super::ExitParams;
    }

    /// Extension notification
    pub struct ExtNotification;
    impl Notification for ExtNotification {
        const METHOD: &'static str = "client/extNotification";
        type Params = super::ExtNotification;
    }
}
