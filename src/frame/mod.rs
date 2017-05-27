use std::borrow::Cow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::str;

pub use self::content::{Content, ExtendedText, ExtendedLink, Comment, Lyrics, Picture, PictureType};
pub use self::timestamp::Timestamp;

use self::flags::Flags;
use ::stream::frame::{self, v2, v3, v4};

use ::tag::{self, Version};

mod content;
#[doc(hidden)]
pub mod flags;
mod timestamp;


/// A structure representing an ID3 frame.
///
/// It is imporant to note that the (Partial)Eq and Hash implementations are based on the ID3 spec.
/// This means that text frames with equal ID's are equal but picture frames with both "APIC" as ID
/// are not because their uniqueness is also defined by their content.
#[derive(Clone, Debug, Eq)]
pub struct Frame {
    /// The frame identifier.
    id: [u8; 4],
    /// The parsed content of the frame.
    #[doc(hidden)]
    pub content: Content,
    /// The frame flags.
    #[doc(hidden)]
    pub flags: Flags,
}

impl PartialEq for Frame {
    fn eq(&self, other: &Frame) -> bool {
        match self.content {
            Content::Text(_) => self.id == other.id,
            _ => {
                self.id == other.id && self.content == other.content
            },
        }
    }
}

impl Hash for Frame {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        match self.content {
            Content::Text(_) => self.id.hash(state),
            _ => {
                self.id.hash(state);
                self.content.hash(state);
            },
        }
    }
}

impl Frame {
    /// Creates a new ID3v2.3 frame with the specified identifier.
    ///
    /// # Panics
    /// If the id's length is not 3 or 4 bytes long or not known.
//    #[deprecated(note = "Use with_content")]
    pub fn new<T: Into<String>>(id: T) -> Frame {
        Frame::with_content(&id.into(), Content::Unknown(Vec::new()))
    }

    /// Creates a frame with the specified ID and content.
    ///
    /// Both ID3v2.2 and >ID3v2.3 IDs are accepted, although they will be converted to ID3v2.3
    /// format.
    ///
    /// # Panics
    /// If the id's length is not 3 or 4 bytes long or not known.
    pub fn with_content(id: &str, content: Content) -> Frame {
        assert!({
            let l = id.bytes().count();
            l == 3 || l == 4
        });
        Frame {
            id: {
                let idv3 = if id.len() == 3 {
                    // ID3v2.3 supports all ID3v2.2 frames, unwrapping should be safe.
                    ::util::convert_id_2_to_3(id).unwrap()
                } else {
                    id
                };
                let mut b = idv3.bytes();
                [
                    b.next().unwrap(),
                    b.next().unwrap(),
                    b.next().unwrap(),
                    b.next().unwrap(),
                ]
            },
            content: content,
            flags: Flags::new(),
        }
    }

    /// Returns the 4-byte ID of this frame.
    pub fn id(&self) -> &str {
        str::from_utf8(&self.id).unwrap()
    }

    /// Returns the ID that is compatible with specified version or None if no ID is available in
    /// that version.
    pub fn id_for_version(&self, version: Version) -> Option<&str> {
        match version {
            Version::Id3v22 => ::util::convert_id_3_to_2(self.id()),
            Version::Id3v23|Version::Id3v24 => Some(str::from_utf8(&self.id).unwrap()),
        }
    }

    /// Returns the content of the frame.
    pub fn content(&self) -> &Content {
        &self.content
    }

    // Getters/Setters
    /// Returns whether the compression flag is set.
    pub fn compression(&self) -> bool {
        self.flags.compression
    }

    /// Sets the compression flag.
    pub fn set_compression(&mut self, compression: bool) {
        self.flags.compression = compression;
        self.flags.data_length_indicator = compression;
    }

    /// Returns whether the tag_alter_preservation flag is set.
    pub fn tag_alter_preservation(&self) -> bool {
        self.flags.tag_alter_preservation
    }

    /// Sets the tag_alter_preservation flag.
    pub fn set_tag_alter_preservation(&mut self, tag_alter_preservation: bool) {
        self.flags.tag_alter_preservation = tag_alter_preservation;
    }

    /// Returns whether the file_alter_preservation flag is set.
    pub fn file_alter_preservation(&self) -> bool {
        self.flags.file_alter_preservation
    }

    /// Sets the file_alter_preservation flag.
    pub fn set_file_alter_preservation(&mut self, file_alter_preservation: bool) {
        self.flags.file_alter_preservation = file_alter_preservation;
    }

    /// Attempts to read a frame from the reader.
    ///
    /// Returns a tuple containing the number of bytes read and a frame. If pading is encountered
    /// then `None` is returned.
    ///
    /// Only reading from versions 2, 3, and 4 is supported. Attempting to read any other version
    /// will return an error with kind `UnsupportedVersion`.
    pub fn read_from<R>(reader: &mut R, version: tag::Version, unsynchronization: bool) -> ::Result<Option<(usize, Frame)>>
        where R: io::Read {
        frame::decode(reader, version, unsynchronization)
    }

    /// Attempts to write the frame to the writer.
    ///
    /// Returns the number of bytes written.
    ///
    /// Only writing to versions 2, 3, and 4 is supported. Attempting to write using any other
    /// version will return an error with kind `UnsupportedVersion`.
    pub fn write_to(&self, writer: &mut Write, version: tag::Version, unsynchronization: bool) -> ::Result<u32> {
        match version {
            tag::Id3v22 => v2::write(writer, self, unsynchronization),
            tag::Id3v23 => v3::write(writer, self, unsynchronization),
            tag::Id3v24 => v4::write(writer, self),
        }
    }

    /// Returns a string representing the parsed content.
    ///
    /// Returns `None` if the parsed content can not be represented as text.
    ///
    /// # Example
    /// ```
    /// use id3::frame::{self, Frame, Content};
    ///
    /// let title_frame = Frame::with_content("TIT2", Content::Text("title".to_owned()));
    /// assert_eq!(&title_frame.text().unwrap()[..], "title");
    ///
    /// let mut txxx_frame = Frame::with_content("TXXX", Content::ExtendedText(frame::ExtendedText {
    ///     description: "description".to_owned(),
    ///     value: "value".to_owned()
    /// }));
    /// assert_eq!(&txxx_frame.text().unwrap()[..], "description: value");
    /// ```
    #[deprecated(note = "Format using fmt::Display")]
    pub fn text(&self) -> Option<Cow<str>> {
        Some(Cow::Owned(format!("{}", self)))
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.content {
            Content::Text(ref content) => write!(f, "{}", content),
            Content::Link(ref content) => write!(f, "{}", content),
            Content::Lyrics(ref content) => write!(f, "{}", content.text),
            Content::ExtendedText(ref content) => write!(f, "{}: {}", content.description, content.value),
            Content::ExtendedLink(ref content) => write!(f, "{}: {}", content.description, content.link),
            Content::Comment(ref content) => write!(f, "{}: {}", content.description, content.text),
            Content::Picture(ref content) => write!(f, "{}: {:?} ({:?})", content.description, content.picture_type, content.mime_type),
            Content::Unknown(ref content) => write!(f, "unknown, {} bytes", content.len()),
        }
    }
}

// Tests {{{
#[cfg(test)]
mod tests {
    use super::*;
    use frame::{Frame, Flags};
    use ::stream::encoding::Encoding;
    use ::stream::unsynch;

    fn u32_to_bytes(n: u32) -> Vec<u8> {
        vec!(((n & 0xFF000000) >> 24) as u8,
             ((n & 0xFF0000) >> 16) as u8,
             ((n & 0xFF00) >> 8) as u8,
             (n & 0xFF) as u8
            )
    }

    /// Parses the provided data and sets the `content` field. If the compression flag is set to
    /// true then decompression will be performed.
    ///
    /// Returns `Err` if the data is invalid for the frame type.
    fn parse_data(frame: &mut Frame, data: &[u8]) -> ::Result<()> {
        frame.content = ::stream::frame::decode_content(io::Cursor::new(data), frame.id(), frame.flags)?;
        Ok(())
    }

    #[test]
    fn test_frame_flags_to_bytes_v3() {
        let mut flags = Flags::new();
        assert_eq!(flags.to_bytes(0x3), vec!(0x0, 0x0));
        flags.tag_alter_preservation = true;
        flags.file_alter_preservation = true;
        flags.read_only = true;
        flags.compression = true;
        flags.encryption = true;
        flags.grouping_identity = true;
        assert_eq!(flags.to_bytes(0x3), vec!(0xE0, 0xE0));
    }

    #[test]
    fn test_frame_flags_to_bytes_v4() {
        let mut flags = Flags::new();
        assert_eq!(flags.to_bytes(0x4), vec!(0x0, 0x0));
        flags.tag_alter_preservation = true;
        flags.file_alter_preservation = true;
        flags.read_only = true;
        flags.grouping_identity = true;
        flags.compression = true;
        flags.encryption = true;
        flags.unsynchronization = true;
        flags.data_length_indicator = true;
        assert_eq!(flags.to_bytes(0x4), vec!(0x70, 0x4F));
    }

    #[test]
    fn test_to_bytes_v2() {
        let id = "TAL";
        let text = "album";
        let encoding = Encoding::UTF16;

        let mut frame = Frame::new(id);

        let mut data = Vec::new();
        data.push(encoding as u8);
        data.extend(::util::string_to_utf16(text).into_iter());

        parse_data(&mut frame, &data[..]).unwrap();

        let mut bytes = Vec::new();
        bytes.extend(id.bytes());
        bytes.extend((&u32_to_bytes(data.len() as u32)[1..]).iter().cloned());
        bytes.extend(data.into_iter());

        let mut writer = Vec::new();
        frame.write_to(&mut writer, tag::Id3v22, false).unwrap();
        assert_eq!(writer, bytes);
    }

    #[test]
    fn test_to_bytes_v3() {
        let id = "TALB";
        let text = "album";
        let encoding = Encoding::UTF16;

        let mut frame = Frame::new(id);

        let mut data = Vec::new();
        data.push(encoding as u8);
        data.extend(::util::string_to_utf16(text).into_iter());

        parse_data(&mut frame, &data[..]).unwrap();

        let mut bytes = Vec::new();
        bytes.extend(id.bytes());
        bytes.extend(u32_to_bytes(data.len() as u32).into_iter());
        bytes.extend([0x00, 0x00].iter().cloned());
        bytes.extend(data.into_iter());

        let mut writer = Vec::new();
        frame.write_to(&mut writer, tag::Id3v23, false).unwrap();
        assert_eq!(writer, bytes);
    }

    #[test]
    fn test_to_bytes_v4() {
        let id = "TALB";
        let text = "album";
        let encoding = Encoding::UTF8;

        let mut frame = Frame::new(id);

        frame.flags.tag_alter_preservation = true;
        frame.flags.file_alter_preservation = true;

        let mut data = Vec::new();
        data.push(encoding as u8);
        data.extend(text.bytes());

        parse_data(&mut frame, &data[..]).unwrap();

        let mut bytes = Vec::new();
        bytes.extend(id.bytes());
        bytes.extend(u32_to_bytes(unsynch::encode_u32(data.len() as u32)).into_iter());
        bytes.extend([0x60, 0x00].iter().cloned());
        bytes.extend(data.into_iter());

        let mut writer = Vec::new();
        frame.write_to(&mut writer, tag::Id3v24, false).unwrap();
        assert_eq!(writer, bytes);
    }
}
