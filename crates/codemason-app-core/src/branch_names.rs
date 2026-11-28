//! Random branch names for the sidebar's `New session on branch`, in
//! Docker's `<adjective>_<surname>` scheme. The generator lives in
//! [`codemason_agent::readable_names`], which also names team members.

pub use codemason_agent::readable_names::{random_branch_name, unused_branch_name};
