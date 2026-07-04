mod normalize;

pub use normalize::{build_formatprint_payload, FormatPrintPayload};

use anyhow::Result;
use hw_collect::collect_inventory;
pub use hw_collect::CollectConfig;
use hw_model::{ComponentInfo, Inventory, MonitorInfo};
use hw_store::bindid::calculate_bindid;
pub use hw_store::component::{ComponentRow, ToRow};
use hw_store::mods::{attach_monitors, system_to_row};

pub async fn collect(config: &CollectConfig) -> Result<Inventory> {
    collect_inventory(config).await
}

pub fn compute_bind_id(inv: &Inventory) -> String {
    let mut keys = Vec::new();
    keys.push(inv.cpu.parsed.get_composite_key());
    for m in &inv.memory {
        keys.push(m.parsed.get_composite_key());
    }
    keys.push(inv.bios.parsed.get_composite_key());
    for m in &inv.monitors {
        keys.push(m.parsed.get_composite_key());
    }
    for s in &inv.storage {
        keys.push(s.parsed.get_composite_key());
    }
    for g in &inv.gpus {
        keys.push(g.parsed.get_composite_key());
    }
    for n in &inv.networks {
        keys.push(n.parsed.get_composite_key());
    }

    calculate_bindid(&keys)
}

pub fn to_component_rows(
    inv: &Inventory,
    machine_sn: Option<&str>,
    bind_id: &str,
) -> Vec<ComponentRow> {
    let sn = machine_sn.map(|s| s.trim()).filter(|s| !s.is_empty());
    let mut rows = Vec::new();

    let mut system_row = system_to_row(&inv.cpu.parsed, &inv.bios.parsed, bind_id);
    system_row = apply_sn(system_row, sn);
    rows.push(system_row);

    let mut board_row = inv.bios.parsed.to_row(bind_id);
    board_row = apply_sn(board_row, sn);
    rows.push(board_row);

    for mem in &inv.memory {
        let row = apply_sn(mem.parsed.to_row(bind_id), sn);
        rows.push(row);
    }

    for sto in &inv.storage {
        let row = apply_sn(sto.parsed.to_row(bind_id), sn);
        rows.push(row);
    }

    let monitor_infos: Vec<MonitorInfo> = inv.monitors.iter().map(|m| m.parsed.clone()).collect();
    for (idx, gpu) in inv.gpus.iter().enumerate() {
        let mut row = gpu.parsed.to_row(bind_id);
        if idx == 0 && !monitor_infos.is_empty() {
            row = attach_monitors(row, &monitor_infos);
        }
        let row = apply_sn(row, sn);
        rows.push(row);
    }

    for net in &inv.networks {
        let row = apply_sn(net.parsed.to_row(bind_id), sn);
        rows.push(row);
    }

    rows
}

fn apply_sn(mut row: ComponentRow, sn: Option<&str>) -> ComponentRow {
    if let Some(sn) = sn {
        if row.fd_SN.is_none() || row.fd_SN.as_deref().map(str::is_empty).unwrap_or(false) {
            row = row.sn(sn);
        }
    }
    row
}
