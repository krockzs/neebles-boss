pub mod dictionary;
pub mod executor;
pub mod request;
pub mod result;

pub use request::LocalInstallerRequest;
pub use result::LocalInstallerResult;

pub fn handle(request: LocalInstallerRequest) -> Result<LocalInstallerResult, String> {
    executor::execute(request)
}
