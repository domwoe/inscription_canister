// Taken from https://github.com/ordinals/ord/blob/master/src/inscriptions/inscription.rs and
// https://github.com/ordinals/ord/blob/master/src/inscriptions/tag.rs
 
use bitcoin::blockdata::{
    opcodes,
    script::{self, PushBytesBuf},
};

use std::{convert::TryInto, mem};

use serde::{Deserialize, Serialize};

const PROTOCOL_ID: [u8; 3] = *b"ord";
const BODY_TAG: [u8; 0] = [];
/// The maximum allowed script size.
pub const MAX_SCRIPT_ELEMENT_SIZE: usize = 520;

#[derive(Copy, Clone)]
pub(crate) enum Tag {
    Pointer,
    #[allow(unused)]
    Unbound,

    ContentType,
    Parent,
    Metadata,
    Metaprotocol,
    ContentEncoding,
    Delegate,
    #[allow(unused)]
    Nop,
}

impl Tag {
    fn is_chunked(self) -> bool {
        matches!(self, Self::Metadata)
    }

    pub(crate) fn bytes(self) -> &'static [u8] {
        match self {
            Self::Pointer => &[2],
            Self::Unbound => &[66],

            Self::ContentType => &[1],
            Self::Parent => &[3],
            Self::Metadata => &[5],
            Self::Metaprotocol => &[7],
            Self::ContentEncoding => &[9],
            Self::Delegate => &[11],
            Self::Nop => &[255],
        }
    }

    pub(crate) fn encode(self, builder: &mut script::Builder, value: &Option<Vec<u8>>) {
        if let Some(value) = value {
            let mut tmp = script::Builder::new();
            mem::swap(&mut tmp, builder);

            if self.is_chunked() {
                for chunk in value.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
                    tmp = tmp
                        .push_slice::<&script::PushBytes>(self.bytes().try_into().unwrap())
                        .push_slice::<&script::PushBytes>(chunk.try_into().unwrap());
                }
            } else {
                tmp = tmp
                    .push_slice::<&script::PushBytes>(self.bytes().try_into().unwrap())
                    .push_slice::<&script::PushBytes>(value.as_slice().try_into().unwrap());
            }

            mem::swap(&mut tmp, builder);
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq, Default)]
pub struct Inscription {
    pub body: Option<Vec<u8>>,
    pub content_encoding: Option<Vec<u8>>,
    pub content_type: Option<Vec<u8>>,
    pub delegate: Option<Vec<u8>>,
    pub duplicate_field: bool,
    pub incomplete_field: bool,
    pub metadata: Option<Vec<u8>>,
    pub metaprotocol: Option<Vec<u8>>,
    pub parent: Option<Vec<u8>>,
    pub pointer: Option<Vec<u8>>,
    pub unrecognized_even_field: bool,
}

impl Inscription {
    pub(crate) fn new(content_type: Option<Vec<u8>>, body: Option<Vec<u8>>) -> Self {
        Self {
            content_type,
            body,
            ..Default::default()
        }
    }

    pub(crate) fn append_reveal_script_to_builder(
        &self,
        mut builder: script::Builder,
    ) -> script::Builder {
        builder = builder
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(PROTOCOL_ID);

        Tag::ContentType.encode(&mut builder, &self.content_type);
        Tag::ContentEncoding.encode(&mut builder, &self.content_encoding);
        Tag::Metaprotocol.encode(&mut builder, &self.metaprotocol);
        Tag::Parent.encode(&mut builder, &self.parent);
        Tag::Delegate.encode(&mut builder, &self.delegate);
        Tag::Pointer.encode(&mut builder, &self.pointer);
        Tag::Metadata.encode(&mut builder, &self.metadata);

        if let Some(body) = &self.body {
            builder = builder.push_slice(BODY_TAG);
            for chunk in body.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
                builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec()).unwrap());
            }
        }

        builder.push_opcode(opcodes::all::OP_ENDIF)
    }
}
