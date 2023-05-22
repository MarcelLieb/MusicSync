use std::collections::HashMap;

use plotters::{
    prelude::{BitMapBackend, ChartBuilder, IntoDrawingArea, LabelAreaPosition, Rectangle, Circle},
    style::{AsRelative, Color, Palette, Palette99, BLACK, WHITE},
};

use super::lights::Event;

pub fn plot(
    data: &HashMap<String, Vec<(u128, Event)>>,
    file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&file, (1280, 960)).into_drawing_area();

    root.fill(&WHITE)?;

    let max = data.iter().fold(0_u128, |acc, (_, vec)| {
        vec.iter().filter(|(t, _)| *t < 10000).last().unwrap_or(&(0, Event::Full(0.0))).0.max(acc)
    });

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Left, (8).percent())
        .set_label_area_size(LabelAreaPosition::Bottom, (4).percent())
        .margin(5)
        .build_cartesian_2d(0..max, 0_u32..6_u32)?;
    chart.configure_mesh().disable_y_mesh().draw()?;

    for (index, key) in data.keys().enumerate() {
        let color = Palette99::pick(index);
        chart
            .draw_series({
                data[key].iter().map(|(time, event)| {
                    match event {
                        Event::Full(y) => (*time, *y),
                        Event::Atmosphere(y, _) => (*time, *y),
                        Event::Note(y, _) => (*time, *y),
                        Event::Drum(y) => (*time, *y),
                        Event::Hihat(y) => (*time, *y),
                    }
                }).filter(|(t, _)| *t < 10000).map(|(t, _v)| Circle::new((t, (index + 1) as u32), 3, &color))
            }
            )?
            .label(key)
            .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));
    }

    chart
        .configure_series_labels()
        .border_style(&BLACK)
        .draw()?;

    root.present()?;

    Ok(())
}
