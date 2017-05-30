extern crate rusttype;

#[allow(non_snake_case)]
pub trait Canvas {
    fn translate(&mut self, x: f64, y: f64) -> &mut Canvas;
    fn fillText(&mut self, text: &str, x: f64, y: f64) -> &mut Canvas;
    fn fillRect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas;
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Canvas;
    fn arc(&mut self, x: f64, y: f64, r: f64, alpha: f64, beta: f64) -> &mut Canvas;
    fn moveTo(&mut self, x: f64, y: f64) -> &mut Canvas;
    fn lineTo(&mut self, x: f64, y: f64) -> &mut Canvas;
    fn setLineDash(&mut self, dash: &Vec<f64>) -> &mut Canvas;
    fn rotate(&mut self, alpha: f64) -> &mut Canvas;
    fn scale(&mut self, scale: f64) -> &mut Canvas;
    fn fillStyle(&mut self, style: &str) -> &mut Canvas;
    fn textAlign(&mut self, align: &str) -> &mut Canvas;
    fn textBaseline(&mut self, baseline: &str) -> &mut Canvas;
    fn lineWidth(&mut self, width: f64) -> &mut Canvas;
    fn strokeStyle(&mut self, style: &str) -> &mut Canvas;
    fn font(&mut self, font: &str) -> &mut Canvas;
    fn save(&mut self) -> &mut Canvas;
    fn restore(&mut self) -> &mut Canvas;
    fn beginPath(&mut self) -> &mut Canvas;
    fn closePath(&mut self) -> &mut Canvas;
    fn fill(&mut self) -> &mut Canvas;
    fn stroke(&mut self) -> &mut Canvas;
    fn clip(&mut self) -> &mut Canvas;

    fn get_font_metrics(&self, scale: f64) -> rusttype::VMetrics;
}
