use anyhow::anyhow;
use anyhow::Result;
use futures_util::stream::{Stream, StreamExt, TryStreamExt};
use log::{debug, error, info, warn};
use rupnp::ssdp::{SearchTarget, URN};
use std::str::FromStr;
use std::time::Duration;

const STOP: [&str; 2] = ["STOPPED", "NO_MEDIA_PRESENT"];
const PAYLOAD_PLAY: &str = r#"
    <InstanceID>0</InstanceID>
    <Speed>1</Speed>
"#;
const AV_TRANSPORT: URN = URN::service("schemas-upnp-org", "AVTransport", 1);

macro_rules! format_device {
    ($device:expr) => {{
        format!(
            "[{}] {} @ {}",
            $device.device_type(),
            $device.friendly_name(),
            $device.url()
        )
    }};
}

#[derive(Debug, Clone)]
pub struct Render {
    /// The UPnP device
    pub device: rupnp::Device,
    /// The AVTransport service
    pub service: rupnp::Service,
}

impl Render {
    pub fn discover(duration_secs: u64) -> Result<Vec<Self>> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(async { Self::_discover(duration_secs).await })
    }

    async fn from_device(device: rupnp::Device) -> Option<Self> {
        debug!(
            "Retrieving AVTransport service from device '{}'",
            format_device!(device)
        );
        match device.find_service(&AV_TRANSPORT) {
            Some(service) => Some(Self {
                device: device.clone(),
                service: service.clone(),
            }),
            None => {
                warn!("No AVTransport service found on {}", device.friendly_name());
                None
            }
        }
    }

    pub async fn _discover(duration_secs: u64) -> Result<Vec<Self>> {
        info!("查找设备, 请等待 {} 秒...", duration_secs);
        let search_target = SearchTarget::URN(AV_TRANSPORT);
        let devices =
            upnp_discover(&search_target, Duration::from_secs(duration_secs), Some(20)).await?;

        pin_utils::pin_mut!(devices);

        let mut renders = Vec::new();

        while let Some(result) = devices.next().await {
            match result {
                Ok(device) => {
                    debug!("找到设备 {}", format_device!(device));
                    if let Some(render) = Self::from_device(device).await {
                        renders.push(render);
                    };
                }
                Err(e) => {
                    debug!("在查找的时候出现一个错误 {}", e);
                }
            }
        }

        Ok(renders)
    }

    pub fn is_stopped(&self) -> bool {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { self._is_stopped().await })
    }

    pub async fn _is_stopped(&self) -> bool {
        let ret = loop {
            match self
                .service
                .action(self.device.url(), "GetTransportInfo", PAYLOAD_PLAY)
                .await
            {
                Ok(ret) => {
                    break ret;
                }
                Err(_) => {
                    error!("状态查询失败正在重试")
                }
            }
        };
        debug!("获取到 ret = {:?}", &ret);
        if ret.is_empty() {
            return true;
        } else if ret.contains_key("CurrentTransportState") {
            let state = ret["CurrentTransportState"].clone();
            debug!("DLNA设备状态{}", &state);
            if STOP.contains(&state.as_str()) {
                return true;
            }
        }
        false
    }

    pub async fn _play(&self, url: &str) -> Result<()> {
        info!("投屏{}", url);

        let payload_setavtransporturi = format!(
            r#"
            <InstanceID>0</InstanceID>
            <CurrentURI>{}</CurrentURI>
            <CurrentURIMetaData>my-dlna</CurrentURIMetaData>
            "#,
            url,
        );

        println!("target1");
        println!("{}", self.device.url());
        let uri = rupnp::http::Uri::from_str(
            format!("{}", self.device.url())
                .replace("http://192.168.1.100:49152/", "http://192.168.1.100:6095/")
                .as_str(),
        )?;
        println!("{}", payload_setavtransporturi.as_str());
        let ret = match self
            .service
            .action(
                &uri,
                "SetAVTransportURI",
                payload_setavtransporturi.as_str(),
            )
            .await
        {
            Ok(ret) => ret,
            Err(e) => {
                return Err(anyhow!("DLNASetAVTransportURIError e={}", e));
            }
        };

        println!("{:?}", ret);

        println!("target2");
        let ret = match self
            .service
            .action(self.device.url(), "Play", PAYLOAD_PLAY)
            .await
        {
            Ok(ret) => ret,
            Err(e) => {
                return Err(anyhow!("DLNAPlayError e={}", e));
            }
        };
        println!("{:?}", ret);
        println!("target3");

        Ok(())
    }
}

impl std::fmt::Display for Render {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "[{}][{}] {} @ {}",
            self.device.device_type(),
            self.service.service_type(),
            self.device.friendly_name(),
            self.device.url()
        )
    }
}

async fn upnp_discover(
    search_target: &SearchTarget,
    timeout: Duration,
    ttl: Option<u32>,
) -> Result<impl Stream<Item = Result<rupnp::Device, rupnp::Error>>> {
    Ok(ssdp_client::search(search_target, timeout, 10, ttl)
        .await?
        .map_err(rupnp::Error::SSDPError)
        .map(|res| Ok(res?.location().parse()?))
        .and_then(rupnp::Device::from_url))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::stdin;
    use std::thread::sleep;
    use std::time::Duration;

    #[tokio::test]
    async fn test() {
        let url = "https://www.w3schools.com/html/movie.mp4";
        println!("将要搜索renders");
        let discovered_devices = loop {
            if let Ok(ret) = Render::_discover(30).await {
                if !ret.is_empty() {
                    break ret;
                }
            }
            println!("搜索不到");
        };
        for (i, render) in discovered_devices.iter().enumerate() {
            println!("[{}]{}", i, render);
        }
        let target_render = discovered_devices[1].clone();
        println!("获取到render = {}", &target_render);
        target_render._play(url).await.unwrap();
    }
}
