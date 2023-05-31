use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::graphene;
use gtk::gsk;
use tdlib::enums::MessageContent;

use crate::tdlib::Message;
use crate::utils::decode_image_from_path;
use crate::utils::spawn;
use crate::Session;

mod imp {

    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::MessageAlbumPhoto)]
    pub(crate) struct MessageAlbumPhoto {
        pub(super) binding: RefCell<Option<gtk::ExpressionWatch>>,
        pub(super) handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub(super) size: Cell<(i32, i32)>,
        #[property(get, set = Self::set_texture, nullable)]
        pub(super) texture: RefCell<Option<gdk::Texture>>,
        #[property(get, set = Self::set_message)]
        pub(super) message: RefCell<Option<Message>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageAlbumPhoto {
        const NAME: &'static str = "MessageAlbumPhoto";
        type Type = super::MessageAlbumPhoto;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("photo");
        }
    }

    impl ObjectImpl for MessageAlbumPhoto {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_overflow(gtk::Overflow::Visible);

            self.obj().connect_scale_factor_notify(|obj| {
                obj.update_photo(obj.imp().message.borrow().as_ref().unwrap());
            });
        }
    }

    impl WidgetImpl for MessageAlbumPhoto {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (photo_width, photo_height) = if let Some(texture) = &*self.texture.borrow() {
                (texture.width(), texture.height())
            } else {
                self.size.get()
            };

            let aspect_ratio = photo_width as f32 / photo_height as f32;

            if for_size == -1 {
                let size = if orientation == gtk::Orientation::Vertical {
                    photo_height
                } else {
                    photo_width
                };
                return (0, size / 4, -1, -1);
            }

            let size = if orientation == gtk::Orientation::Vertical {
                let width = for_size as f32;
                width / aspect_ratio
            } else {
                let heigth = for_size as f32;
                heigth * aspect_ratio
            } as i32;

            let v = &Some(0);

            // if v.is_none() {
            //     panic()
            // }

            let size = if orientation == gtk::Orientation::Vertical {
                let width = for_size as f32;
                width / aspect_ratio
            } else {
                let heigth = for_size as f32;
                heigth * aspect_ratio
            } as i32;

            (0, size, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(texture) = &*self.texture.borrow() else {
                return;
            };

            let widget = self.obj();

            let bounds = {
                let width = widget.width() as f32;
                let height = widget.height() as f32;

                graphene::Rect::new(0.0, 0.0, width, height)
            };

            if bounds.area() <= 0.0 {
                return;
            }

            let texture_bounds = {
                let width = texture.width() as f32;
                let height = texture.height() as f32;

                graphene::Rect::new(0.0, 0.0, width, height)
            };

            let scale_h = bounds.width() / texture_bounds.width();
            let scale_v = bounds.height() / texture_bounds.height();

            let s = scale_h.max(scale_v);
            let mut scaled_bounds = texture_bounds.scale(s, s);

            let d_x = (bounds.width() - scaled_bounds.width()) * 0.5;
            let d_y = (bounds.height() - scaled_bounds.height()) * 0.5;
            scaled_bounds.offset(d_x, d_y);

            let texture_bounds = scaled_bounds;

            snapshot.push_clip(&bounds);
            snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &scaled_bounds);
            snapshot.pop();
        }
    }

    impl MessageAlbumPhoto {
        fn set_texture(&self, texture: Option<gdk::Texture>) {
            self.texture.replace(texture);
            self.obj().queue_draw();
        }

        fn set_message(&self, message: Message) {
            let obj = self.obj();

            let imp = self;

            if imp.message.borrow().as_ref() == Some(&message) {
                return;
            }

            if let Some(binding) = imp.binding.take() {
                binding.unwatch();
            }

            if let Some(old_message) = imp.message.take() {
                let handler_id = imp.handler_id.take().unwrap();
                old_message.disconnect(handler_id);
            }

            imp.message.replace(Some(message));

            let message_ref = imp.message.borrow();
            let message = message_ref.as_ref().unwrap();

            // Load photo
            let handler_id =
                message.connect_content_notify(clone!(@weak obj => move |message, _| {
                    obj.update_photo(message);
                }));
            imp.handler_id.replace(Some(handler_id));
            obj.update_photo(message);

            obj.notify("message");
        }
    }
}

glib::wrapper! {
    pub(crate) struct MessageAlbumPhoto(ObjectSubclass<imp::MessageAlbumPhoto>)
        @extends gtk::Widget;
}

impl MessageAlbumPhoto {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn update_photo(&self, message: &Message) {
        if let MessageContent::MessagePhoto(mut data) = message.content().0 {
            let imp = self.imp();
            // Choose the right photo size based on the screen scale factor.
            // See https://core.telegram.org/api/files#image-thumbnail-types for more
            // information about photo sizes.
            // let photo_size = if self.scale_factor() > 2 {
            //     data.photo.sizes.pop().unwrap()
            // } else {
            //     let type_ = if self.scale_factor() > 1 { "y" } else { "x" };

            //     match data.photo.sizes.iter().position(|s| s.r#type == type_) {
            //         Some(pos) => data.photo.sizes.swap_remove(pos),
            //         None => data.photo.sizes.pop().unwrap(),
            //     }
            // };

            let photo_size = data.photo.sizes.pop().unwrap();

            imp.size.set((photo_size.width, photo_size.height));

            if photo_size.photo.local.is_downloading_completed {
                self.load_photo(photo_size.photo.local.path);
            } else {
                self.set_texture(data.photo.minithumbnail.and_then(|m| {
                    gdk::Texture::from_bytes(&glib::Bytes::from_owned(glib::base64_decode(&m.data)))
                        .ok()
                }));

                let file_id = photo_size.photo.id;
                let session = message.chat().session();
                spawn(clone!(@weak self as obj, @weak session => async move {
                    obj.download_photo(file_id, &session).await;
                }));
            }
        }
    }

    async fn download_photo(&self, file_id: i32, session: &Session) {
        match session.download_file(file_id).await {
            Ok(file) => {
                self.load_photo(file.local.path);
            }
            Err(e) => {
                log::warn!("Failed to download a photo: {e:?}");
            }
        }
    }

    fn message_id(&self) -> i64 {
        self.imp()
            .message
            .borrow()
            .as_ref()
            .map(|m| m.id())
            .unwrap_or_default()
    }

    fn load_photo(&self, path: String) {
        let message_id = self.message_id();

        spawn(clone!(@weak self as obj => async move {
            let result = gio::spawn_blocking(move || decode_image_from_path(&path))
                .await
                .unwrap();

            // Check if the current message id is the same as the one at
            // the time of the request. It may be changed because of the
            // ListView recycling while decoding the image.
            if obj.message_id() != message_id {
                return;
            }

            match result {
                Ok(texture) => {
                    obj.set_texture(Some(texture));
                }
                Err(e) => {
                    log::warn!("Error decoding a photo: {e:?}");
                }
            }
        }));
    }
}
