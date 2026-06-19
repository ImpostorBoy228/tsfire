pub mod layout;
pub mod paint;
pub mod cpu_renderer;

#[cfg(feature = "gpu")]
pub mod window;
#[cfg(not(feature = "gpu"))]
pub mod window {
    use crate::ui_shit::cpu_renderer::CpuRenderer;
    use crate::ui_shit::paint;

    pub fn run(list: paint::DisplayList) -> Result<(), Box<dyn std::error::Error>> {
        paint::dump_display_list(&list);

        let cw = (list.content_size.width as usize).max(800).min(1920);
        let ch = (list.content_size.height as usize).max(600).min(1080);

        let mut window = match minifb::Window::new(
            "tsfire (CPU)",
            cw,
            ch,
            minifb::WindowOptions {
                resize: true,
                ..minifb::WindowOptions::default()
            },
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("no-gpu window unavailable ({}), headless mode", e);
                return Ok(());
            }
        };
        window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

        let mut renderer = CpuRenderer::new(cw as u32, ch as u32);
        renderer.render(&list);

        while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
            let (ww, wh) = window.get_size();
            if ww != renderer.width as usize || wh != renderer.height as usize {
                renderer.resize(ww as u32, wh as u32);
                renderer.render(&list);
            }
            window
                .update_with_buffer(&renderer.buffer, renderer.width as usize, renderer.height as usize)?;
        }
        Ok(())
    }
}

#[cfg(feature = "gpu")]
pub mod display_renderer;
