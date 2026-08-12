use crate::app_core::recognition::region::Region;

/// Простая DBNet-подобная обработка probability map.
///
/// На этом этапе мы не делаем полноценный polygon/unclip.
/// Наша задача — получить отдельные connected components
/// и превратить их в bounding boxes.
pub fn postprocess(data: &[f32], width: usize, height: usize, threshold: f32) -> Vec<Region> {
    if data.len() != width * height {
        return Vec::new();
    }

    let mut visited = vec![false; data.len()];
    let mut regions = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;

            if visited[index] {
                continue;
            }

            visited[index] = true;

            if data[index] <= threshold {
                continue;
            }

            // BFS connected component.
            let mut queue = std::collections::VecDeque::new();
            queue.push_back((x, y));

            let mut min_x = x;
            let mut min_y = y;
            let mut max_x = x;
            let mut max_y = y;

            let mut pixel_count = 0usize;

            while let Some((cx, cy)) = queue.pop_front() {
                pixel_count += 1;

                min_x = min_x.min(cx);
                min_y = min_y.min(cy);
                max_x = max_x.max(cx);
                max_y = max_y.max(cy);

                // 4-connected neighbours.
                let neighbours = [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ];

                for (nx, ny) in neighbours {
                    if nx >= width || ny >= height {
                        continue;
                    }

                    let neighbour_index = ny * width + nx;

                    if visited[neighbour_index] {
                        continue;
                    }

                    visited[neighbour_index] = true;

                    if data[neighbour_index] > threshold {
                        queue.push_back((nx, ny));
                    }
                }
            }

            // Игнорируем слишком маленькие компоненты.
            //
            // Сейчас это только диагностический фильтр.
            if pixel_count < 20 {
                continue;
            }

            let region_width = max_x - min_x + 1;
            let region_height = max_y - min_y + 1;

            // Игнорируем совсем маленькие области.
            if region_width < 3 || region_height < 3 {
                continue;
            }

            regions.push(
                Region::new(min_x as i32, min_y as i32, region_width as u32, region_height as u32)
            );
        }
    }

    regions
}
