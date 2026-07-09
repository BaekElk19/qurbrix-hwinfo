pub mod arch;
pub mod cpu_vendor;
pub mod gpu_vendor;
pub mod pnp;

pub use arch::normalize_arch;
pub use cpu_vendor::{clean_loongson_name, infer_cpu_vendor_from_name, normalize_cpu_vendor_id};
pub use gpu_vendor::{normalize_gpu_vendor, normalize_gpu_vendor_id};
pub use pnp::lookup_pnp_manufacturer;
