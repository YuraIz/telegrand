use gtk::glib;
use gtk::glib::DateTime;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;

use crate::tdlib::Message;

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "ContentChatHistoryItemType")]
pub(crate) enum ChatHistoryItemType {
    Message(Message),
    MediaGroup(Vec<Message>),
    DayDivider(DateTime),
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub(crate) struct ChatHistoryItem {
        pub(super) type_: OnceCell<ChatHistoryItemType>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ChatHistoryItem {
        const NAME: &'static str = "ContentChatHistoryItem";
        type Type = super::ChatHistoryItem;
    }

    impl ObjectImpl for ChatHistoryItem {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecBoxed::builder::<ChatHistoryItemType>("type")
                    .write_only()
                    .construct_only()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "type" => {
                    let type_ = value.get::<ChatHistoryItemType>().unwrap();
                    self.type_.set(type_).unwrap();
                }
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub(crate) struct ChatHistoryItem(ObjectSubclass<imp::ChatHistoryItem>);
}

impl ChatHistoryItem {
    pub(crate) fn for_message(message: Message) -> Self {
        let type_ = ChatHistoryItemType::Message(message);
        glib::Object::builder().property("type", type_).build()
    }

    pub(crate) fn for_media_group(media_group: Vec<Message>) -> Self {
        let type_ = ChatHistoryItemType::MediaGroup(media_group);
        glib::Object::builder().property("type", type_).build()
    }

    pub(crate) fn for_day_divider(day: DateTime) -> Self {
        let type_ = ChatHistoryItemType::DayDivider(day);
        glib::Object::builder().property("type", type_).build()
    }

    pub(crate) fn type_(&self) -> &ChatHistoryItemType {
        self.imp().type_.get().unwrap()
    }

    pub(crate) fn message(&self) -> Option<&Message> {
        if let ChatHistoryItemType::Message(message) = self.type_() {
            Some(message)
        } else {
            None
        }
    }

    pub(crate) fn media_group(&self) -> Option<&[Message]> {
        if let ChatHistoryItemType::MediaGroup(media_group) = self.type_() {
            Some(media_group)
        } else {
            None
        }
    }

    pub(crate) fn message_timestamp(&self) -> Option<DateTime> {
        let date = match self.type_() {
            ChatHistoryItemType::Message(message) => message.date(),
            ChatHistoryItemType::MediaGroup(media_group) => media_group.first()?.date(),
            _ => return None,
        };

        Some(
            glib::DateTime::from_unix_utc(date.into())
                .and_then(|t| t.to_local())
                .unwrap(),
        )
    }
}
