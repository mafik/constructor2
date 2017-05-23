use canvas::Canvas;

struct JSONCanvas {
    count: i32,
    cmds: String,
}

impl JSONCanvas {
    fn new() -> JSONCanvas {
        JSONCanvas {
            count: 0,
            cmds: "[".to_string(),
        }
    }
    fn append(&mut self, cmd: String) -> &mut JSONCanvas {
        if self.count > 0 {
            self.cmds += ",";
        }
        self.count += 1;
        self.cmds += &cmd;
        self
    }
    fn serialize(self) -> String {
        self.cmds + "]"
    }
}

impl Canvas for JSONCanvas {
    fn translate(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"translate","x":{},"y":{}}}"#, x, y))
    }
    fn fillText(&mut self, text: &str, x: f64, y: f64) -> &mut Canvas {
        // TODO: escape text with serde_json::to_string
        unimplemented!();
        self.append(format!(r#"{{"type":"fillText","text":"{}","x":{},"y":{}}}"#, text, x, y))
    }
    fn fillRect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"fillRect","x":{},"y":{},"width":{},"height":{}}}"#, x, y, w, h))
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"rect","x":{},"y":{},"width":{},"height":{}}}"#, x, y, w, h))
    }
    fn arc(&mut self, x: f64, y: f64, r: f64, alpha: f64, beta: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"arc","x":{},"y":{},"r":{},"alpha":{},"beta":{}}}"#, x, y, r, alpha, beta))
    }
    fn moveTo(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"moveTo","x":{},"y":{}}}"#, x, y))
    }
    fn lineTo(&mut self, x: f64, y: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"lineTo","x":{},"y":{}}}"#, x, y))
    }
    fn setLineDash(&mut self, dash: &Vec<f64>) -> &mut Canvas {
        unimplemented!();
    }
    fn rotate(&mut self, alpha: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"rotate","alpha":{}}}"#, alpha))
    }
    fn fillStyle(&mut self, style: &str) -> &mut Canvas {
        unimplemented!();
    }
    fn textAlign(&mut self, align: &str) -> &mut Canvas {
        unimplemented!();
    }
    fn textBaseline(&mut self, baseline: &str) -> &mut Canvas {
        unimplemented!();
    }
    fn lineWidth(&mut self, width: f64) -> &mut Canvas {
        self.append(format!(r#"{{"type":"lineWidth","width":{}}}"#, width))
    }
    fn strokeStyle(&mut self, style: &str) -> &mut Canvas {
        unimplemented!();
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
    use super::JSONCanvas;
    use ::std::f64::consts::PI;

    #[test]
    fn translate() {
        assert_eq!(JSONCanvas::new().translate(-1., 1.).serialize(),
                   r#"[{"type":"translate","x":-1,"y":1}]"#);
    }

    #[test]
    fn fillText() {
        assert_eq!(JSONCanvas::new().fillText("a b c", 1., 2.).serialize(),
                   r#"[{"type":"fillText","text":"a b c","x":1,"y":2}]"#);
    }

    #[test]
    fn fillText_edge_cases() {
        assert_eq!(JSONCanvas::new().fillText("\\\"\"\\", 1., 2.).serialize(),
                   r#"[{"type":"fillText","text":"\\\"\"\\","x":1,"y":2}]"#);
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
