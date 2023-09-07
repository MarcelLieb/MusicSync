use std::collections::HashMap;

use plotters::{
    prelude::{
        BitMapBackend, ChartBuilder, Circle, IntoDrawingArea, LabelAreaPosition, PathElement,
        Rectangle, SeriesLabelPosition,
    },
    series::LineSeries,
    style::{AsRelative, Color, Palette, Palette99, BLACK, RED, WHITE},
};

use super::lights::Event;

const TIME_WINDOW: u128 = 10000;

pub fn plot(
    onsets: &HashMap<String, Vec<(u128, Event)>>,
    raw_data: &[f32],
    time_resolution: u32,
    file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&file, (1920, 1080)).into_drawing_area();

    root.fill(&WHITE)?;

    let max = (raw_data.len() as u128 * time_resolution as u128).min(TIME_WINDOW);

    let mut circle_chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Bottom, (4).percent())
        .margin(20)
        .build_cartesian_2d(0..max, 0_u32..6_u32)?;
    circle_chart
        .configure_mesh()
        .disable_y_mesh()
        .x_desc("time in ms")
        .draw()?;

    let mut graph_chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Bottom, (4).percent())
        .margin(20)
        .build_cartesian_2d(0..max, 0_f32..1_f32)?;

    graph_chart
        .configure_mesh()
        .disable_mesh()
        .disable_axes()
        .draw()?;

    let mut keys = onsets
        .keys()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>();
    keys.sort();

    let data_max: HashMap<String, f32> = onsets
        .iter()
        .map(|(key, vec)| {
            (
                key.to_string(),
                vec.iter()
                    .filter(|(t, _)| *t < TIME_WINDOW)
                    .filter(|(t, _)| *t > 20) // Start is usually a unwanted click
                    .map(|(_, event)| event)
                    .map(|event| match event {
                        Event::Full(y)
                        | Event::Atmosphere(y, _)
                        | Event::Note(y, _)
                        | Event::Drum(y)
                        | Event::Hihat(y)
                        | Event::Raw(y) => *y,
                    })
                    .fold(f32::EPSILON, f32::max),
            )
        })
        .collect();

    for (index, key) in keys.iter().enumerate() {
        let color = Palette99::pick(index);
        circle_chart
            .draw_series({
                onsets[key]
                    .iter()
                    .map(|(time, event)| match event {
                        Event::Full(y)
                        | Event::Atmosphere(y, _)
                        | Event::Note(y, _)
                        | Event::Drum(y)
                        | Event::Hihat(y)
                        | Event::Raw(y) => (*time, *y),
                    })
                    .map(|(time, y)| (time, y / data_max[key]))
                    .filter(|(t, _)| *t < TIME_WINDOW)
                    .filter(|(t, _)| *t > 20) // Start is usually a unwanted click
                    .flat_map(|(t, v)| {
                        [
                            Circle::new(
                                (t, (-(index as i32) + 5) as u32),
                                25.0 * v,
                                color.mix(0.8),
                            ),
                            Circle::new(
                                (t, (-(index as i32) + 5) as u32),
                                2.0,
                                color.mix(0.1).filled(),
                            ),
                        ]
                    })
            })?
            .label(key)
            .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));
    }

    let raw_max = raw_data.iter().fold(f32::EPSILON, |acc, x| acc.max(*x));
    graph_chart
        .draw_series(LineSeries::new(
            raw_data
                .iter()
                .enumerate()
                .map(|(t, y)| ((t as u32 * time_resolution + 20) as u128, y / raw_max * 0.5))
                .filter(|(t, _)| *t < TIME_WINDOW),
            &RED.mix(0.8),
        ))?
        .label("Onset function")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));
    circle_chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(WHITE)
        .border_style(BLACK)
        .draw()?;

    root.present()?;

    Ok(())
}
