//! One integration-test binary for the crate: each file under tests/ would
//! otherwise be linked separately, which is the slow part of a CI build.

mod lazy_reload;
mod runtime_exit;
mod wedged_extension;
