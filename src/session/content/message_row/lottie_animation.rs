use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use rlottie;

use flate2::read::GzDecoder;
use std::io;
use std::io::prelude::*;

mod imp {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
    };

    #[derive(Default)]
    pub struct LottieAnimation {
        pub frame_num: Cell<usize>,
        pub totalframe: Cell<usize>,
        pub animation: RefCell<Option<rlottie::Animation>>,
        pub intrinsic: Cell<(i32, i32, f64)>,
        pub cache: RefCell<Vec<gdk::MemoryTexture>>,
        pub texture: RefCell<Option<gdk::MemoryTexture>>,
        pub last_cache_use: Cell<Option<std::time::Instant>>,
        pub player_source_id: Cell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LottieAnimation {
        const NAME: &'static str = "ContentLottieAnimation";
        type Type = super::LottieAnimation;
        type ParentType = gtk::MediaFile;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for LottieAnimation {}
    impl MediaFileImpl for LottieAnimation {
        fn open(&self, media_file: &Self::Type) {
            if let Some(file) = media_file.file() {
                let path = file.path().unwrap();
                let animation = match path
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                {
                    "json" => rlottie::Animation::from_file(path).expect("Can't open animation"),
                    "tgs" => {
                        let data = file.load_contents(gio::Cancellable::NONE).unwrap().0;

                        let mut gz = GzDecoder::new(&*data);

                        let mut buf = String::new();

                        gz.read_to_string(&mut buf).expect("can't read file");

                        rlottie::Animation::from_data(
                            buf,
                            path.file_name().unwrap().to_str().unwrap(), // path.file_name().unwrap().to_str().unwrap(),
                            "",
                        )
                        .expect("Can't create tgs animation")
                    }
                    _ => panic!("unsupporded file type"),
                };

                let was_playing = media_file.is_playing();
                media_file.pause();

                // self.cache.borrow_mut().clear();
                self.frame_num.set(0);

                let size = rlottie::Size::new(208, 208);
                let framerate = animation.framerate();
                // let totalframe = (animation.totalframe() + 1) / 2;
                let totalframe = animation.totalframe();
                _ = self.animation.replace(Some(animation));

                let (width, height) = (size.width as i32, size.height as i32);
                let aspect_ratio = width as f64 / height as f64;

                self.intrinsic.set((width, height, aspect_ratio));
                self.totalframe.set(totalframe);

                if was_playing {
                    media_file.play();
                }
            }
        }
    }
    impl MediaStreamImpl for LottieAnimation {
        fn play(&self, media_stream: &Self::Type) -> bool {
            // nothing
            true
        }

        fn pause(&self, media_stream: &Self::Type) {
            // nothing
        }
    }

    impl gdk::subclass::paintable::PaintableImpl for LottieAnimation {
        fn flags(&self, _: &Self::Type) -> gdk::PaintableFlags {
            gdk::PaintableFlags::SIZE
        }

        fn intrinsic_width(&self, _: &Self::Type) -> i32 {
            self.intrinsic.get().0
        }

        fn intrinsic_height(&self, _: &Self::Type) -> i32 {
            self.intrinsic.get().1
        }

        fn intrinsic_aspect_ratio(&self, _: &Self::Type) -> f64 {
            self.intrinsic.get().2
        }

        fn snapshot(&self, obj: &Self::Type, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let total_frame = self.totalframe.get();
            let frame_num = (self.frame_num.get() + total_frame - 1) % total_frame;

            let mut cache = self.cache.borrow_mut();

            if let Some(texture) = &cache.get(frame_num) {
                // &*self.texture.borrow() {
                texture.snapshot(snapshot, width, height);
                self.last_cache_use.set(Some(std::time::Instant::now()));
            }

            if obj.is_playing() {
                glib::timeout_add_once(
                    std::time::Duration::from_secs_f64(1.0 / 60.0),
                    clone!(@weak obj =>  move || {
                        obj.imp().setup_next_frame();
                        obj.invalidate_contents();
                    }),
                );

                if frame_num == 0 || frame_num == total_frame / 2 {
                    glib::timeout_add_local_once(
                        std::time::Duration::from_secs(2),
                        clone!(@weak obj =>  move || {
                                let imp = obj.imp();
                                if let Some(instatnt) = imp.last_cache_use.get() {
                                    if instatnt.elapsed() > std::time::Duration::from_secs_f32(0.5) {
                                    dbg!(imp.cache.take());
                                    obj.imp().frame_num.set(0);
                                    imp.setup_next_frame();
                                }
                            }
                        }),
                    );
                }
            }
        }
    }

    impl LottieAnimation {
        fn setup_next_frame(&self) {
            let mut cache = self.cache.borrow_mut();
            let frame_num = self.frame_num.get();

            if cache.len() != self.totalframe.get() {
                if let Some(ref mut animation) = *self.animation.borrow_mut() {
                    let (width, height, _) = self.intrinsic.get();
                    let mut surface =
                        rlottie::Surface::new(rlottie::Size::new(width as usize, height as usize));
                    animation.render(frame_num, &mut surface);

                    let data = surface.data();

                    let mut data = unsafe {
                        std::slice::from_raw_parts_mut(data.as_ptr() as *mut u8, data.len() * 4)
                    };

                    let data = glib::Bytes::from_owned(data.to_owned());

                    let texture = gdk::MemoryTexture::new(
                        width,
                        height,
                        gdk::MemoryFormat::B8g8r8a8,
                        &data,
                        width as usize * 4,
                    );

                    cache.push(texture);

                    // self.texture.replace(Some(texture));
                }
            }
            self.frame_num.set((frame_num + 1) % self.totalframe.get());
        }
    }
}

glib::wrapper! {
    pub struct LottieAnimation(ObjectSubclass<imp::LottieAnimation>)
        @extends gtk::MediaFile, gtk::MediaStream,
        @implements gdk::Paintable;
}

impl LottieAnimation {
    pub fn from_file(file: gio::File) -> Self {
        glib::Object::new(&[("file", &file)]).expect("Failed to create LottieAnimation")
    }

    pub fn from_filename(path: &str) -> Self {
        let file = gio::File::for_path(path);
        Self::from_file(file)
    }
}

unsafe impl Sync for LottieAnimation {}
unsafe impl Send for LottieAnimation {}
