use adw::prelude::*;
use glib::clone;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use tdlib::enums::MessageContent;
use tdlib::types::File;

use crate::session::components::{RltOverlay, StickerPreview};
use crate::session::content::message_row::{MessageBase, MessageBaseImpl, MessageIndicators};
use crate::tdlib::Message;

use super::base::MessageBaseExt;

mod imp {
    use crate::utils::spawn;

    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;
    use tdlib::functions;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-animated-emoji.ui")]
    pub(crate) struct MessageAnimatedEmoji {
        pub(super) message: RefCell<Option<Message>>,
        #[template_child]
        pub(super) gtk_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) click: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub(super) bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub(super) indicators: TemplateChild<MessageIndicators>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageAnimatedEmoji {
        const NAME: &'static str = "ContentMessageAnimatedEmoji";
        type Type = super::MessageAnimatedEmoji;
        type ParentType = MessageBase;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("messagesticker");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageAnimatedEmoji {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "message",
                    "Message",
                    "The message represented by this row",
                    Message::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "message" => self.obj().set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => self.message.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.click
                .connect_released(clone!(@to-owned self as imp => move |_, _, _, _| {
                    if let Some(animation) = imp.bin.child() {
                        if let Ok(animation) = animation.downcast::<rlt::Animation>() {
                            if !animation.is_playing() {
                                animation.play();
                            }

                            let Some(message) = &*imp.message.borrow() else {return};

                            let chat_id = message.chat().id();
                            let message_id = message.id();
                            let client_id = message.chat().session().client_id();
                            let outgoing = message.is_outgoing();

                            let session = message.chat().session();

                            spawn(clone!(@to-owned imp => async move {

                                if let Ok(tdlib::enums::Sticker::Sticker(sticker)) = functions::click_animated_emoji_message(chat_id, message_id, client_id).await {
                                    let file = sticker.sticker;

                                    let imp = imp.obj();

                                let append_animation = move |imp: &MessageAnimatedEmoji, path: &str| {
                                    let animation = rlt::Animation::from_filename(&path);

                                    let shift_x = if !outgoing {
                                        animation.add_css_class("mirrored");
                                        -100
                                    } else {
                                        100
                                    } + glib::random_int_range(-10, 10);

                                    let shift_y = glib::random_int_range(-10, 10);

                                    RltOverlay::append(&imp.bin.get(), animation, 300, shift_x, shift_y);
                                };

                                if file.local.is_downloading_completed {
                                    append_animation(&imp.imp(), &file.local.path);
                                } else {
                                    let (sender, receiver) =
                                        glib::MainContext::sync_channel::<File>(Default::default(), 5);

                                    receiver.attach(
                                        None,
                                        clone!(@to-owned imp => @default-return glib::Continue(false), move |file| {
                                            if file.local.is_downloading_completed {
                                                append_animation(imp.imp(), &file.local.path);
                                            }

                                            glib::Continue(true)
                                        }),
                                    );

                                    session.download_file(file.id, sender);
                                }
                                }
                            }))
                        }
                    }
                }));
        }
    }

    impl WidgetImpl for MessageAnimatedEmoji {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            const SIZE: i32 = 120;
            let min = self.gtk_box.measure(orientation, for_size).0;
            if orientation == gtk::Orientation::Horizontal {
                (SIZE, SIZE, -1, -1)
            } else {
                (SIZE + min, SIZE + min, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.gtk_box.allocate(width, height, baseline, None);
        }
    }
    impl MessageBaseImpl for MessageAnimatedEmoji {}
}

glib::wrapper! {
    pub(crate) struct MessageAnimatedEmoji(ObjectSubclass<imp::MessageAnimatedEmoji>)
        @extends gtk::Widget, MessageBase;
}

impl MessageBaseExt for MessageAnimatedEmoji {
    type Message = Message;

    fn set_message(&self, message: Self::Message) {
        let imp = self.imp();

        if imp.message.borrow().as_ref() == Some(&message) {
            return;
        }

        imp.indicators.set_message(message.clone().upcast());

        if let MessageContent::MessageAnimatedEmoji(data) = message.content().0 {
            let data = data.animated_emoji;

            let sticker = data.sticker.unwrap();

            let preview = StickerPreview::new(sticker.outline.clone());
            self.imp().bin.set_child(Some(&preview));

            let file = sticker.sticker;

            let looped = matches!(
                sticker.full_type,
                tdlib::enums::StickerFullType::CustomEmoji(_)
            );

            if file.local.is_downloading_completed {
                self.load_sticker(&file.local.path, looped)
            } else {
                let (sender, receiver) =
                    glib::MainContext::sync_channel::<File>(Default::default(), 5);

                receiver.attach(
                    None,
                    clone!(@weak self as obj => @default-return glib::Continue(false), move |file| {
                        if file.local.is_downloading_completed {
                            obj.load_sticker(&file.local.path, looped);
                        }

                        glib::Continue(true)
                    }),
                );

                message.chat().session().download_file(file.id, sender);
            }
        }

        imp.message.replace(Some(message));

        self.notify("message");
    }
}

impl MessageAnimatedEmoji {
    fn load_sticker(&self, path: &str, looped: bool) {
        let path = path.to_owned();

        let animation = rlt::Animation::from_filename(&path);
        animation.set_loop(looped);
        animation.use_cache(looped);
        animation.play();

        self.imp().bin.set_child(Some(&animation));
    }
}
