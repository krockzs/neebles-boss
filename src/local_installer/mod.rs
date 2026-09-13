pub mod dictionary;
pub mod distribution;
pub mod executor;
pub mod request;
pub mod result;

pub use dictionary::{DictionaryAudit, ResolvedOperation};
pub use request::LocalInstallerRequest;
pub use result::LocalInstallerResult;

pub fn handle(
    request: LocalInstallerRequest,
) -> Result<LocalInstallerResult, String> {
    executor::execute(request)
}

pub fn resolve_only(
    request: &LocalInstallerRequest,
) -> Result<ResolvedOperation, String> {
    dictionary::resolve(request)
}

pub fn audit_dictionary() -> Result<DictionaryAudit, String> {
    dictionary::audit_dictionary()
}
