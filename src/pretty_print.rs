use xml::{EmitterConfig, ParserConfig};
use xml::reader::XmlEvent;

pub fn to_writer_pretty<W>(writer: &mut W, buf: &[u8]) -> std::io::Result<usize>
  where
      W: std::io::Write,
{
  let reader = ParserConfig::new()
      .trim_whitespace(true)
      .ignore_comments(false)
      .create_reader(buf);

  let mut writer = EmitterConfig::new()
      .perform_indent(true)
      .normalize_empty_elements(true)
      .autopad_comments(false)
      .write_document_declaration(false)
      .create_writer(writer);

  for event in reader {
    if let Ok(XmlEvent::StartDocument {..}) = event {
      continue
    }
    if let Some(event) = event.map_err(to_io)?.as_writer_event() {
      writer.write(event).map_err(to_io)?;
    }
  }
  Ok(buf.len())
}

fn to_io<E>(e: E) -> std::io::Error
  where
      E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
  std::io::Error::new(std::io::ErrorKind::Other, e)
}