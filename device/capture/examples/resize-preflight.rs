//! Read-only X11/RandR inventory for diagnosing resize preflight failures.
//! No image, input, window title, credential or mode-change requests are made.
use std::error::Error;
use x11rb::{
    connection::Connection,
    protocol::{randr::ConnectionExt as _, xproto::ConnectionExt as _},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("resize preflight failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    eprintln!("stage: connect configured DISPLAY using service Xauthority");
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    eprintln!("stage: RandR version");
    let version = connection.randr_query_version(1, 3)?.reply()?;
    println!("RandR {}.{}", version.major_version, version.minor_version);
    eprintln!("stage: screen resources");
    let resources = connection.randr_get_screen_resources(root)?.reply()?;
    println!(
        "configTimestamp={} timestamp={} outputs={} crtcs={}",
        resources.config_timestamp,
        resources.timestamp,
        resources.outputs.len(),
        resources.crtcs.len()
    );
    for crtc in &resources.crtcs {
        eprintln!("stage: CRTC {crtc} info");
        let info = connection
            .randr_get_crtc_info(*crtc, resources.config_timestamp)?
            .reply()?;
        println!(
            "crtc={crtc} status={} mode={} x={} y={} width={} height={} outputs={:?}",
            u8::from(info.status),
            info.mode,
            info.x,
            info.y,
            info.width,
            info.height,
            info.outputs
        );
    }
    for output in &resources.outputs {
        eprintln!("stage: output {output} info");
        let info = connection
            .randr_get_output_info(*output, resources.config_timestamp)?
            .reply()?;
        println!(
            "output={output} status={} crtc={} modes={:?}",
            u8::from(info.status),
            info.crtc,
            info.modes
        );
    }
    eprintln!("stage: root GetGeometry");
    let geometry = connection.get_geometry(root)?.reply()?;
    println!(
        "root width={} height={} depth={}",
        geometry.width, geometry.height, geometry.depth
    );
    eprintln!("stage: legacy RandR GetScreenInfo");
    let screen = connection.randr_get_screen_info(root)?.reply()?;
    println!(
        "legacy sizeId={} sizes={}",
        screen.size_id,
        screen.sizes.len()
    );
    for (index, size) in screen.sizes.iter().enumerate() {
        println!(
            "legacy size={index} width={} height={} mmWidth={} mmHeight={}",
            size.width, size.height, size.mwidth, size.mheight
        );
    }
    println!("read-only resize preflight completed");
    Ok(())
}
