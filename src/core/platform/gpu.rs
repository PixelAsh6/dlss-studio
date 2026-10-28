use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory: usize,
    pub is_rtx_40: bool,
    #[serde(default)]
    pub family: String,
}

/// NVIDIA GPU generation by name ("RTX 3060", "GTX 1650", ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuFamily {
    Turing,
    Ampere,
    Ada,
    Blackwell,
    Pascal,
    Other,
}

impl GpuFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            GpuFamily::Turing => "turing",
            GpuFamily::Ampere => "ampere",
            GpuFamily::Ada => "ada",
            GpuFamily::Blackwell => "blackwell",
            GpuFamily::Pascal => "pascal",
            GpuFamily::Other => "other",
        }
    }
}

pub fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    unsafe {
        let factory: Result<IDXGIFactory1, _> = CreateDXGIFactory1();
        if let Ok(factory) = factory {
            let mut index = 0;
            while let Ok(adapter) = factory.EnumAdapters1(index) {
                if let Ok(desc) = adapter.GetDesc1() {
                    let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
                    let name = String::from_utf16_lossy(&desc.Description[..len]);

                    // Ignore Microsoft Basic Render Driver
                    if desc.VendorId != 0x1414 {
                        let is_rtx_40 = is_ada_lovelace(&name, desc.VendorId, desc.DeviceId);
                        let family = gpu_family(&name, desc.VendorId);
                        gpus.push(GpuInfo {
                            name,
                            vendor_id: desc.VendorId,
                            device_id: desc.DeviceId,
                            dedicated_video_memory: desc.DedicatedVideoMemory,
                            is_rtx_40,
                            family: family.as_str().to_string(),
                        });
                    }
                }
                index += 1;
            }
        }
    }

    gpus
}

/// Series number from the first model number after `marker`
/// ("RTX 3060 Laptop GPU" -> 3, "GTX 1650" -> 1).
fn series_after(upper: &str, marker: &str) -> Option<u32> {
    let idx = upper.find(marker)?;
    let rest = &upper[idx + marker.len()..];
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok().map(|n| n / 1000)
}

pub fn gpu_family(name: &str, vendor_id: u32) -> GpuFamily {
    if vendor_id != 0x10de {
        return GpuFamily::Other;
    }
    let upper = name.to_uppercase();
    let series = series_after(&upper, "RTX")
        .or_else(|| series_after(&upper, "GTX"))
        .or_else(|| series_after(&upper, "MX"));
    match series {
        Some(5) => GpuFamily::Blackwell,
        Some(4) => GpuFamily::Ada,
        Some(3) => GpuFamily::Ampere,
        Some(2) => GpuFamily::Turing,
        Some(1) | Some(0) => GpuFamily::Pascal,
        _ => GpuFamily::Other,
    }
}

pub fn family_display(family: &str) -> String {
    match family {
        "turing" => "Turing (RTX 20)".to_string(),
        "ampere" => "Ampere (RTX 30)".to_string(),
        "ada" => "Ada (RTX 40)".to_string(),
        "blackwell" => "Blackwell (RTX 50)".to_string(),
        "pascal" => "Pascal (GTX 10/16)".to_string(),
        _ => "Other".to_string(),
    }
}

pub fn is_ada_lovelace(name: &str, vendor_id: u32, device_id: u32) -> bool {
    if vendor_id == 0x10de {
        if (0x2600..=0x28ff).contains(&device_id) {
            return true;
        }
    }

    let upper = name.to_uppercase();
    (upper.contains("RTX 40") || upper.contains("RTX 4000"))
        && !upper.contains("RTX 4000 TURING")
}
