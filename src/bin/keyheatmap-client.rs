use keyheatmap::*;

use tokio::fs;
use xmltree::{Element, XMLNode};

static TEMPLATE: &str = include_str!(r"../../resources/template_plain.svg");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map = load_hashmap().await;

    let mut highest_count = 0;
    let mut highest_time = 0;

    for value in map.values() {
        if value.count > highest_count {
            highest_count = value.count
        }
        if value.time_ms > highest_time {
            highest_time = value.time_ms
        }
    }

    println!(
        "highest count: {}x\nhighest time: {}.{}s",
        highest_count,
        highest_time / 1000,
        highest_time % 1000
    );

    let highest_count = highest_count as f32;
    let highest_time = highest_time as f32;

    //let svg = fs::read_to_string("template_plain.svg")?;
    let mut root1 = Element::parse(TEMPLATE.as_bytes())?;
    let mut root2 = root1.clone();

    find_by_id_mut(&mut root1, "(# of presses)");
    find_by_id_mut(&mut root2, "(time)");

    if root1.attributes.get("id").map(String::as_str) == Some("BG") {
        for child in &mut root1.children {
            if let XMLNode::Element(title) = child {
                if title.name == "title" {
                    title.children.clear();
                    title.children.push(XMLNode::Text("hello".to_string()));
                }
            }
        }
    }

    for (key, value) in &map {
        let key_str = format!("{:?}", key);
        let count_f32 = value.count as f32 / highest_count;
        let time_f32 = value.time_ms as f32 / highest_time;
        set_path_fill(&mut root1, key_str.as_str(), count_f32);
        set_path_fill(&mut root2, key_str.as_str(), time_f32);
    }

    let mut out = Vec::new();
    root1.write(&mut out)?;
    fs::write("keyheatmap_count.svg", &out).await?;

    out.clear();
    root2.write(&mut out)?;
    fs::write("keyheatmap_time.svg", &out).await?;

    println!("Saved files keyheatmap_time.svg and keyheatmap_count.svg");

    Ok(())
}

fn set_path_fill(elem: &mut Element, target_id: &str, opacity: f32) {
    if elem.name == "path" && elem.attributes.get("id").map(String::as_str) == Some(target_id) {
        elem.attributes.insert(
            "style".into(),
            format!("fill:{};fill-opacity:1", heatmap_color(opacity)),
        );
    }

    for child in &mut elem.children {
        if let XMLNode::Element(child) = child {
            set_path_fill(child, target_id, opacity);
        }
    }
}

fn find_by_id_mut<'a>(elem: &'a mut Element, append: &str) -> bool {
    if elem.attributes.get("id").map(String::as_str) == Some("title") {
        elem.children.push(XMLNode::Text(append.into()));
        return true;
    }

    for child in &mut elem.children {
        if let XMLNode::Element(child) = child {
            if find_by_id_mut(child, append) {
                return true;
            }
        }
    }

    false
}

fn heatmap_color(value: f32) -> String {
    let h = (1.0 - value.clamp(0.0, 1.0)) * 240.0;
    let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);

    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}
