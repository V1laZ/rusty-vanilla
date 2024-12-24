use super::osu_api::{Beatmap, Score};
use skia_safe::{
    surfaces, BlendMode, Canvas, Color, Data, EncodedImageFormat, Font, FontMgr, Image, Matrix,
    Paint, Path, Point, Rect,
};

// Constants for layout
const PADDING: f32 = 15.0;
const CANVAS_WIDTH: f32 = 600.0;
const CELL_HEIGHT: f32 = 90.0;

const INTER_FONT: &[u8] = include_bytes!(".././fonts/Inter_18pt-Regular.ttf");
const INTER_FONT_BOLD: &[u8] = include_bytes!(".././fonts/Inter_18pt-Bold.ttf");

fn calc_text_width(font: &Font, text: &str) -> f32 {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    font.measure_str(text, Some(&paint)).0
}

fn create_rounded_rect_path(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Path {
    let mut path = Path::new();
    path.move_to(Point::new(x + radius, y));
    path.line_to(Point::new(x + width - radius, y));
    path.quad_to(Point::new(x + width, y), Point::new(x + width, y + radius));
    path.line_to(Point::new(x + width, y + height - radius));
    path.quad_to(
        Point::new(x + width, y + height),
        Point::new(x + width - radius, y + height),
    );
    path.line_to(Point::new(x + radius, y + height));
    path.quad_to(
        Point::new(x, y + height),
        Point::new(x, y + height - radius),
    );
    path.line_to(Point::new(x, y + radius));
    path.quad_to(Point::new(x, y), Point::new(x + radius, y));
    path
}

fn draw_profile_image(canvas: &Canvas, avatar: Vec<u8>, x: f32, y: f32, size: f32, radius: f32) {
    let path = create_rounded_rect_path(x, y, size, size, radius);

    let image_data = Data::new_copy(&avatar);

    if let Some(image) = Image::from_encoded(image_data) {
        let scale_x = size / image.width() as f32;
        let scale_y = size / image.height() as f32;

        let mut matrix = Matrix::scale((scale_x, scale_y));
        matrix.post_translate(Point::new(x, y));

        if let Some(shader) = image.to_shader(
            (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
            skia_safe::FilterMode::Linear,
            &matrix,
        ) {
            let mut paint = Paint::default();
            paint.set_shader(shader);

            canvas.save();
            canvas.clip_path(&path, None, Some(true));

            canvas.draw_rect(Rect::from_xywh(x, y, size, size), &paint);

            canvas.restore();
        }
    }
}

fn draw_username(canvas: &Canvas, font: &Font, username: &str, x: f32, y: f32) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(true);

    canvas.draw_str(username, (x, y), font, &paint);
}

fn draw_mods(canvas: &Canvas, font: &Font, mods: &str, x: f32, y: f32) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(true);

    let text_width = calc_text_width(font, mods);

    canvas.draw_str(mods, (x - text_width, y), font, &paint);
}

fn draw_score_with_combo(
    canvas: &Canvas,
    font: &Font,
    score: i64,
    combo: i32,
    max_combo: &str,
    x: f32,
    y: f32,
) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(true);

    let score_formatted = score
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(",");

    let text = format!("{} ({}x / {}x)", score_formatted, combo, max_combo);
    canvas.draw_str(&text, (x, y), font, &paint);
}

fn draw_accuracy(canvas: &Canvas, font: &Font, accuracy: f32, x: f32, y: f32) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(true);

    let text = format!("{:.2}%", accuracy * 100.0);

    let text_width = calc_text_width(font, &text);

    canvas.draw_str(&text, (x - text_width, y), font, &paint);
}

fn draw_ended_at_date(canvas: &Canvas, font: &Font, ended_at: &str, x: f32, y: f32) {
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);
    paint.set_anti_alias(true);

    // Parse and format date
    let formatted_date = {
        let date = ended_at.split('T').next().unwrap_or("");
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 {
            let year = parts[0];
            let month = parts[1].trim_start_matches('0');
            let day = parts[2].trim_start_matches('0');
            format!("{}. {}. {}", day, month, year)
        } else {
            ended_at.to_string()
        }
    };

    let text_width = calc_text_width(font, &formatted_date);
    canvas.draw_str(&formatted_date, (x - text_width, y), font, &paint);
}

fn draw_rank(canvas: &Canvas, font: &Font, rank: &str, x: f32, y: f32) {
    let (rank_to_display, rank_color) = match rank {
        "XH" => ("SS", Color::from_rgb(239, 239, 239)), // #efefef
        "X" => ("SS", Color::from_rgb(255, 226, 76)),   // #ffe24c
        "SH" => ("S", Color::from_rgb(239, 239, 239)),  // #efefef
        "S" => ("S", Color::from_rgb(255, 226, 76)),    // #ffe24c
        "A" => ("A", Color::from_rgb(115, 229, 57)),    // #73e539
        "B" => ("B", Color::from_rgb(57, 143, 230)),    // #398fe6
        "C" => ("C", Color::from_rgb(186, 57, 230)),    // #ba39e6
        "D" => ("D", Color::from_rgb(230, 57, 57)),     // #e63939
        _ => ("?", Color::from_rgb(230, 57, 57)),       // Default red
    };

    let mut paint = Paint::default();
    paint.set_color(rank_color);
    paint.set_anti_alias(true);

    canvas.draw_str(rank_to_display, (x, y), &font, &paint);
}

fn draw_statistics(canvas: &Canvas, font: &Font, statistics: (i32, i32, i32), x: f32, y: f32) {
    let (ok, meh, miss) = statistics;

    // Draw numbers
    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);

    // Draw dots
    let mut dot_paint = Paint::default();
    dot_paint.set_anti_alias(true);

    // First group (green)
    let x1 = x - 40.0;
    let ok_str = ok.to_string();
    let ok_width = calc_text_width(font, &ok_str);
    canvas.draw_str(&ok_str, (x1 - ok_width - 10.0, y), &font, &paint);
    dot_paint.set_color(Color::GREEN);
    canvas.draw_circle((x1, y - 4.2), 4.0, &dot_paint);

    // Second group (yellow)
    let x2 = x + 10.0;
    let meh_str = meh.to_string();
    let meh_width = calc_text_width(font, &meh_str);
    canvas.draw_str(&meh_str, (x2 - meh_width - 10.0, y), &font, &paint);
    dot_paint.set_color(Color::YELLOW);
    canvas.draw_circle((x2, y - 4.2), 4.0, &dot_paint);

    // Third group (red)
    let x3 = x + 60.0;
    let miss_str = miss.to_string();
    let miss_width = calc_text_width(font, &miss_str);
    canvas.draw_str(&miss_str, (x3 - miss_width - 10.0, y), &font, &paint);
    dot_paint.set_color(Color::RED);
    canvas.draw_circle((x3, y - 4.2), 4.0, &dot_paint);
}

fn draw_background(canvas: &Canvas, cover_bytes: &Vec<u8>, canvas_width: f32, canvas_height: f32) {
    let image = Image::from_encoded(Data::new_copy(&cover_bytes))
        .expect("Failed to create image from cover");

    let scale_x = canvas_width / image.width() as f32;
    let scale_y = canvas_height / image.height() as f32;
    let scale = scale_x.max(scale_y);

    // Calculate centered position
    let scaled_width = image.width() as f32 * scale;
    let scaled_height = image.height() as f32 * scale;
    let x = (canvas_width - scaled_width) / 2.0;
    let y = (canvas_height - scaled_height) / 2.0;

    let mut paint = Paint::default();
    paint.set_alpha_f(0.2);
    paint.set_blend_mode(BlendMode::SrcOver);

    let mut matrix = Matrix::scale((scale, scale));
    matrix.post_translate((x, y));

    let shader = image.to_shader(
        (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
        skia_safe::FilterMode::Linear,
        &matrix,
    );

    if let Some(shader) = shader {
        paint.set_shader(shader);
        canvas.draw_rect(Rect::from_xywh(x, y, scaled_width, scaled_height), &paint);
    }
}

pub fn generate_leaderboard(
    leaderboard: Vec<Score>,
    avatars: Vec<Vec<u8>>,
    beatmap_info: &Beatmap,
) -> Vec<u8> {
    let canvas_height = PADDING + CELL_HEIGHT * leaderboard.len() as f32;

    let font_mgr = FontMgr::new();

    let typeface_default = font_mgr
        .new_from_data(INTER_FONT, None)
        .expect("Failed to load Inter Font");
    let typeface_bold = font_mgr
        .new_from_data(INTER_FONT_BOLD, None)
        .expect("Failed to load Inter Font Bold");

    let default_font = Font::from_typeface(&typeface_default, 18.0);
    let bold_font = Font::from_typeface(&typeface_bold, 18.0);
    let smaller_font = Font::from_typeface(&typeface_default, 14.0);
    let mut surface = surfaces::raster_n32_premul((CANVAS_WIDTH as i32, canvas_height as i32))
        .expect("Failed to create surface");

    let mut canvas = surface.canvas();
    canvas.clear(Color::BLACK);

    draw_background(canvas, &beatmap_info.cover, CANVAS_WIDTH, canvas_height);

    leaderboard.iter().enumerate().for_each(|(i, score)| {
        let row_y = PADDING + i as f32 * CELL_HEIGHT;

        draw_profile_image(&mut canvas, avatars[i].clone(), PADDING, row_y, 70.0, 10.0);

        draw_username(
            &mut canvas,
            &bold_font,
            &score.user.username,
            PADDING + 80.0,
            row_y + CELL_HEIGHT * 0.2,
        );

        draw_mods(
            &mut canvas,
            &default_font,
            &score
                .mods
                .iter()
                .map(|m| m.acronym.clone())
                .collect::<String>(),
            CANVAS_WIDTH - PADDING,
            row_y + CELL_HEIGHT * 0.2,
        );

        draw_score_with_combo(
            &mut canvas,
            &default_font,
            score.legacy_total_score,
            score.max_combo,
            &beatmap_info.max_combo,
            PADDING + 80.0,
            row_y + CELL_HEIGHT / 2.0,
        );

        draw_accuracy(
            &mut canvas,
            &default_font,
            score.accuracy,
            CANVAS_WIDTH - PADDING,
            row_y + CELL_HEIGHT / 2.0,
        );

        draw_ended_at_date(
            &mut canvas,
            &default_font,
            &score.ended_at,
            CANVAS_WIDTH - PADDING,
            row_y + CELL_HEIGHT * 0.75,
        );

        draw_rank(
            &mut canvas,
            &bold_font,
            &score.rank,
            PADDING + 80.0,
            row_y + CELL_HEIGHT * 0.75,
        );

        draw_statistics(
            &mut canvas,
            &smaller_font,
            (
                score.statistics.ok.unwrap_or(0),
                score.statistics.meh.unwrap_or(0),
                score.statistics.miss.unwrap_or(0),
            ),
            CANVAS_WIDTH / 2.0,
            row_y + CELL_HEIGHT * 0.75,
        );
    });

    // Save the output as bytes vector
    let image = surface.image_snapshot();
    let data = image
        .encode_to_data(EncodedImageFormat::PNG)
        .expect("Failed to encode image");

    data.as_bytes().to_vec()
}
