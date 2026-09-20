//! Host geometry. Cell counts are independent from raster sampling density.

#[derive(Clone, Copy)]
pub struct Geometry {
    pub columns: u32,
    pub rows: u32,
    pub cell_aspect: f64,
}

impl Geometry {
    pub fn current() -> Self {
        let mut geometry = Self {
            columns: 80,
            rows: 24,
            cell_aspect: 0.5,
        };
        #[cfg(unix)]
        {
            // TIOCGWINSZ is read-only; zero pixel dimensions are common. Try
            // stdout first, then stdin for callers that redirect only stdout.
            for fd in [libc::STDOUT_FILENO, libc::STDIN_FILENO] {
                let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
                if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0
                    && ws.ws_col > 0
                    && ws.ws_row > 0
                {
                    geometry.columns = ws.ws_col.into();
                    geometry.rows = ws.ws_row.into();
                    if ws.ws_xpixel > 0 && ws.ws_ypixel > 0 {
                        geometry.cell_aspect = (ws.ws_xpixel as f64 / ws.ws_col as f64)
                            / (ws.ws_ypixel as f64 / ws.ws_row as f64);
                    }
                    break;
                }
            }
        }
        geometry
    }

    pub fn fit(self, width: f64, height: f64) -> (u32, u32) {
        let max_cols = self.columns.saturating_sub(2).max(1);
        let max_rows = self.rows.saturating_sub(2).max(1);
        let aspect = height / width * self.cell_aspect;
        let columns = (max_cols as f64)
            .min(max_rows as f64 / aspect)
            .floor()
            .max(1.) as u32;
        let rows = (columns as f64 * aspect)
            .ceil()
            .min(max_rows as f64)
            .max(1.) as u32;
        (columns, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tall_scene_shrinks_both_dimensions_to_fit() {
        let terminal = Geometry {
            columns: 102,
            rows: 22,
            cell_aspect: 0.5,
        };
        assert_eq!(terminal.fit(100., 100.), (40, 20));
        assert_eq!(terminal.fit(200., 100.), (80, 20));
        assert_eq!(terminal.fit(500., 100.), (100, 10));
    }
}
