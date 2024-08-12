# 一个通过dlna投屏url到电视的api

## api

pub fn discover()->Result<Vec<Render>> 获取设备列表

pub struct RenderPlay{

  render: Render,

  name: String,

}

impl RenderPlay{

  pub fn new(render: Render)->Self

  pub fn play(&mut self, url: &str)->Result<()> 播放视频

  pub fn is_stopped(&self)->bool 检测状态，如果失败会无限重试

  pub fn name(&self)->String 获取设备名

}