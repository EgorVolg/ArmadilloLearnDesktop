#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct OcrBox {
    pub points: [OcrPoint; 4],
    pub confidence: f32,
    pub text: String,
}

impl OcrBox {
    pub fn bounding_rect(&self) -> (f32, f32, f32, f32) {
        let min_x = self
            .points
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min);

        let min_y = self
            .points
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);

        let max_x = self
            .points
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);

        let max_y = self
            .points
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);

        (min_x, min_y, max_x, max_y)
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        point_in_convex_quad(x, y, &self.points)
    }
}

fn point_in_convex_quad(x: f32, y: f32, points: &[OcrPoint; 4]) -> bool {
    let mut has_positive = false;
    let mut has_negative = false;

    for i in 0..4 {
        let a = points[i];
        let b = points[(i + 1) % 4];

        let cross = (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);

        if cross > 0.0 {
            has_positive = true;
        } else if cross < 0.0 {
            has_negative = true;
        }

        if has_positive && has_negative {
            return false;
        }
    }

    true
}
