extern crate serde_json;
extern crate rusttype;

use std::sync::Arc;
use canvas::Canvas;
use self::rusttype::Font;

pub struct JsonCanvas<'a> {
    count: i32,
    cmds: String,
    font: Arc<Font<'a>>,
}

impl<'a> JsonCanvas<'a> {
    pub fn new(font: Arc<Font<'a>>) -> JsonCanvas<'a> {
        JsonCanvas {
            count: 0,
            cmds: "[".to_string(),
            font: font,
        }
    }
    fn append(&mut self, cmd: String) -> &mut JsonCanvas<'a> {
        if self.count > 0 {
            self.cmds += ",";
        }
        self.count += 1;
        self.cmds += &cmd;
        self
    }
    pub fn serialize(self) -> String {
        self.cmds + "]"
    }
}

impl<'a> Canvas for JsonCanvas<'a> {
    fn get_font_metrics(&self, scale: f64) -> rusttype::VMetrics {
        self.font.v_metrics(rusttype::Scale {
            x: scale as f32,
            y: scale as f32,
        })
    }
    fn translate(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"translate","x":{},"y":{}}}"#, x, y))
    }
    fn fillText(&mut self, text: &str, x: f64, y: f64) -> &mut Canvas {
        let t = serde_json::to_string(text).unwrap();
        self.append(format!(
            r#"{{"type":"fillText","text":{},"x":{},"y":{}}}"#,
            t,
            x,
            y
        ))
    }
    fn fillRect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas {
        self.append(format!(
            r#"{{"type":"fillRect","x":{},"y":{},"w":{},"h":{}}}"#,
            x,
            y,
            w,
            h
        ))
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas {
        self.append(format!(
            r#"{{"type":"rect","x":{},"y":{},"w":{},"h":{}}}"#,
            x,
            y,
            w,
            h
        ))
    }
    fn arc(
        &mut self,
        x: f64,
        y: f64,
        r: f64,
        alpha: f64,
        beta: f64,
        clockwise: bool,
    ) -> &mut Canvas {
        self.append(format!(
            r#"{{"type":"arc","x":{},"y":{},"r":{},"alpha":{},"beta":{},"clockwise":{}}}"#,
            x,
            y,
            r,
            alpha,
            beta,
            clockwise
        ))
    }
    fn ellipse(
        &mut self,
        x: f64,
        y: f64,
        rx: f64,
        ry: f64,
        rotation: f64,
        alpha: f64,
        beta: f64,
        anticlockwise: bool,
    ) -> &mut Canvas {
        self.append(format!(r#"{{"type":"ellipse","x":{},"y":{},"rx":{},"ry":{},"rotation":{},"alpha":{},"beta":{},"anticlockwise":{}}}"#,
                            x,
                            y,
                            rx,
                            ry,
                            rotation,
                            alpha,
                            beta,
                            anticlockwise))
    }
    fn moveTo(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"moveTo","x":{},"y":{}}}"#, x, y))
    }
    fn lineTo(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"lineTo","x":{},"y":{}}}"#, x, y))
    }
    fn setLineDash(&mut self, dash: &Vec<f64>) -> &mut Canvas {
        let d = serde_json::to_string(dash).unwrap();
        self.append(format!(r#"{{"type":"setLineDash","val":{}}}"#, d))
    }
    fn rotate(&mut self, alpha: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"rotate","val":{}}}"#, alpha))
    }
    fn scale(&mut self, scale: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"scale","val":{}}}"#, scale))
    }
    fn fillStyle(&mut self, style: &str) -> &mut Canvas {
        let s = serde_json::to_string(style).unwrap();
        self.append(format!(r#"{{"type":"fillStyle","val":{}}}"#, s))
    }
    fn textAlign(&mut self, align: &str) -> &mut Canvas {
        let a = serde_json::to_string(align).unwrap();
        self.append(format!(r#"{{"type":"textAlign","val":{}}}"#, a))
    }
    fn textBaseline(&mut self, baseline: &str) -> &mut Canvas {
        let b = serde_json::to_string(baseline).unwrap();
        self.append(format!(r#"{{"type":"textBaseline","val":{}}}"#, b))
    }
    fn lineWidth(&mut self, width: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"lineWidth","val":{}}}"#, width))
    }
    fn strokeStyle(&mut self, style: &str) -> &mut Canvas {
        let s = serde_json::to_string(style).unwrap();
        self.append(format!(r#"{{"type":"strokeStyle","val":{}}}"#, s))
    }
    fn font(&mut self, font: &str) -> &mut Canvas {
        let val = serde_json::to_string(font).unwrap();
        self.append(format!(r#"{{"type":"font","val":{}}}"#, val))
    }
    fn save(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"save"}"#.to_owned())
    }
    fn restore(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"restore"}"#.to_owned())
    }
    fn beginPath(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"beginPath"}"#.to_owned())
    }
    fn closePath(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"closePath"}"#.to_owned())
    }
    fn fill(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"fill"}"#.to_owned())
    }
    fn stroke(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"stroke"}"#.to_owned())
    }
    fn clip(&mut self) -> &mut Canvas {
        self.append(r#"{"type":"clip"}"#.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::Canvas;
    use super::JsonCanvas;
    use std::f64::consts::PI;

    #[test]
    fn translate() {
        assert_eq!(
            JsonCanvas::new().translate(-1., 1.).serialize(),
            r#"[{"type":"translate","x":-1,"y":1}]"#
        );
    }

    #[test]
    fn fillText() {
        assert_eq!(
            JsonCanvas::new().fillText("a b c", 1., 2.).serialize(),
            r#"[{"type":"fillText","text":"a b c","x":1,"y":2}]"#
        );
    }

    #[test]
    fn fillText_edge_cases() {
        assert_eq!(
            JsonCanvas::new().fillText("\\\"\"\\", 1., 2.).serialize(),
            r#"[{"type":"fillText","text":"\\\"\"\\","x":1,"y":2}]"#
        );
    }

    //            .fillText(text, x, y)
    //            .fillRect(x, y, w, h)
    //            .rect(x, y, w, h)
    //            .arc(x, y, r, alpha, beta)
    //            .moveTo(x, y)
    //            .lineTo(x, y)
    //            .setLineDash(vec![5, 10, 15])
    //            .rotate(PI)
    //            .fillStyle("a")
    //            .textAlign("b")
    //            .lineWidth("c")
    //            .strokeStyle("d")
    //            .save()
    //            .restore()
    //            .beginPath()
    //            .closePath()
    //            .fill()
    //            .stroke()
    //            .clip()

}
