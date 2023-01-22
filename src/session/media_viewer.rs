use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use tdlib::enums::MessageContent;

use crate::session::components::ScaleRevealer;
use crate::tdlib::Message;

mod imp {
    use super::*;
    use glib::clone;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(string = r#"
    <interface>
      <template class="MediaViewer" parent="GtkWidget">
        <property name="layout-manager">
          <object class="GtkBoxLayout">
            <property name="orientation">vertical</property>
          </object>
        </property>
        <child>
          <object class="GtkHeaderBar">
            <child type="start">
              <object class="GtkButton">
                <property name="action-name">media-viewer.go-back</property>
                <property name="icon-name">go-previous-symbolic</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="ComponentsScaleRevealer" id="revealer">
            <property name="vexpand">True</property>
            <child>
              <object class="GtkPicture" id="picture"/>
            </child>
          </object>
        </child>
      </template>
    </interface>
    "#)]
    pub(crate) struct MediaViewer {
        #[template_child]
        pub(super) revealer: TemplateChild<ScaleRevealer>,
        #[template_child]
        pub(super) picture: TemplateChild<gtk::Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaViewer {
        const NAME: &'static str = "MediaViewer";
        type Type = super::MediaViewer;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.install_action("media-viewer.go-back", None, move |widget, _, _| {
                widget.go_back();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MediaViewer {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            self.revealer
                .connect_transition_done(clone!(@weak obj => move |revealer| {
                    if !revealer.reveals_child() {
                        obj.set_visible(false);
                    }
                }));
        }
    }

    impl WidgetImpl for MediaViewer {}
}

glib::wrapper! {
    pub(crate) struct MediaViewer(ObjectSubclass<imp::MediaViewer>)
        @extends gtk::Widget;
}

impl MediaViewer {
    pub(crate) fn open_media(&self, message: Message, source_widget: &impl IsA<gtk::Widget>) {
        let imp = self.imp();

        if let MessageContent::MessagePhoto(data) = message.content().0 {
            imp.picture.set_paintable(
                data.photo
                    .minithumbnail
                    .and_then(|m| {
                        gdk::Texture::from_bytes(&glib::Bytes::from_owned(glib::base64_decode(
                            &m.data,
                        )))
                        .ok()
                    })
                    .as_ref(),
            );
        }

        self.set_visible(true);

        imp.revealer.set_source_widget(Some(source_widget));
        imp.revealer.set_reveal_child(true);
    }

    fn go_back(&self) {
        self.imp().revealer.set_reveal_child(false);
    }
}
