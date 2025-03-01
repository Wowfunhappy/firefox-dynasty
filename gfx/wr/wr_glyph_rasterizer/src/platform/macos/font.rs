/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use api::{ColorF, ColorU, FontKey, FontRenderMode, FontSize, GlyphDimensions};
use api::{FontInstanceFlags, FontVariation, NativeFontHandle};
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::url::{CFURL, kCFURLPOSIXPathStyle};
use core_graphics::base::{kCGImageAlphaNoneSkipFirst, kCGImageAlphaPremultipliedFirst};
use core_graphics::base::{kCGBitmapByteOrder32Little};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::context::{CGBlendMode, CGTextDrawingMode};
use core_graphics::data_provider::CGDataProvider;
use core_graphics::font::{CGFont, CGGlyph};
use core_graphics::geometry::{CGAffineTransform, CGPoint, CGSize};
use core_graphics::geometry::{CG_AFFINE_TRANSFORM_IDENTITY, CGRect};
use core_text;
use core_text::font::{CTFont, CTFontRef};
use core_text::font_descriptor::kCTFontDefaultOrientation;
use core_text::font_descriptor::kCTFontURLAttribute;
use euclid::default::Size2D;
use crate::gamma_lut::{ColorLut, GammaLut};
use crate::rasterizer::{FontInstance, FontTransform, GlyphKey};
use crate::rasterizer::{GlyphFormat, GlyphRasterError, GlyphRasterResult, RasterizedGlyph};
use crate::types::FastHashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

const INITIAL_CG_CONTEXT_SIDE_LENGTH: u32 = 32;

pub struct FontContext {
    cg_fonts: FastHashMap<FontKey, CGFont>,
    // Table mapping a sized font key with variations to its instantiated CoreText font.
    ct_fonts: FastHashMap<(FontKey, FontSize, Vec<FontVariation>), CTFont>,
    #[allow(dead_code)]
    graphics_context: GraphicsContext,
    #[allow(dead_code)]
    gamma_lut: GammaLut,
}

// core text is safe to use on multiple threads and non-shareable resources are
// all hidden inside their font context.
unsafe impl Send for FontContext {}

struct GlyphMetrics {
    rasterized_left: i32,
    #[allow(dead_code)]
    rasterized_descent: i32,
    rasterized_ascent: i32,
    rasterized_width: i32,
    rasterized_height: i32,
    advance: f32,
}

// There are a number of different OS prefs that control whether or not
// requesting font smoothing actually results in subpixel AA. This gets even
// murkier in newer macOS versions that deprecate subpixel AA, with the prefs
// potentially interacting and overriding each other. In an attempt to future-
// proof things against any new prefs or interpretation of those prefs in
// future macOS versions, we do a check here to request font smoothing and see
// what result it actually gives us much like Skia does. We need to check for
// each of three potential results and process them in the font backend in
// distinct ways:
// 1) subpixel AA (differing RGB channels) with dilation
// 2) grayscale AA (matching RGB channels) with dilation, a compatibility mode
// 3) grayscale AA without dilation as if font smoothing was not requested
// We can discern between case 1 and the rest by checking if the subpixels differ.
// We can discern between cases 2 and 3 by rendering with and without smoothing
// and comparing the two to determine if there was some dilation.
// This returns the actual FontRenderMode needed to support each case, if any.
fn determine_font_smoothing_mode() -> Option<FontRenderMode> {
    let mut smooth_context = CGContext::create_bitmap_context(
        None,
        12,
        12,
        8,
        12 * 4,
        &CGColorSpace::create_device_rgb(),
        kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little,
    );
    smooth_context.set_should_smooth_fonts(true);
    smooth_context.set_should_antialias(true);
    smooth_context.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);
    let mut gray_context = CGContext::create_bitmap_context(
        None,
        12,
        12,
        8,
        12 * 4,
        &CGColorSpace::create_device_rgb(),
        kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little,
    );
    gray_context.set_should_smooth_fonts(false);
    gray_context.set_should_antialias(true);
    gray_context.set_rgb_fill_color(1.0, 1.0, 1.0, 1.0);

    // Autorelease pool for CTFont
    objc::rc::autoreleasepool(|| {
        // Lucida Grande 12 is the default fallback font in Firefox
        let ct_font = core_text::font::new_from_name("Lucida Grande", 12.).unwrap();
        let point = CGPoint { x: 0., y: 0. };
        let glyph = 'X' as CGGlyph;
        ct_font.draw_glyphs(&[glyph], &[point], smooth_context.clone());
        ct_font.draw_glyphs(&[glyph], &[point], gray_context.clone());
    });

    let mut mode = None;
    for (smooth, gray) in smooth_context.data().chunks(4).zip(gray_context.data().chunks(4)) {
        if smooth[0] != smooth[1] || smooth[1] != smooth[2] {
            return Some(FontRenderMode::Subpixel);
        }
        if smooth[0] != gray[0] || smooth[1] != gray[1] || smooth[2] != gray[2] {
            mode = Some(FontRenderMode::Alpha);
        }
    }
    return mode;
}

// We cache the font smoothing mode globally, rather than storing it in each FontContext,
// to avoid having to determine this redundantly in each context and to avoid needing to
// lock them to access this setting in prepare_font.
lazy_static! {
    static ref FONT_SMOOTHING_MODE: Option<FontRenderMode> = determine_font_smoothing_mode();
}

fn get_glyph_metrics(
    ct_font: &CTFont,
    transform: Option<&CGAffineTransform>,
    glyph: CGGlyph,
    x_offset: f64,
    y_offset: f64,
    extra_width: f64,
) -> GlyphMetrics {
    let mut bounds = ct_font.get_bounding_rects_for_glyphs(kCTFontDefaultOrientation, &[glyph]);

    if bounds.origin.x.is_nan() || bounds.origin.y.is_nan() || bounds.size.width.is_nan() ||
        bounds.size.height.is_nan()
    {
        // If an unexpected glyph index is requested, core text will return NaN values
        // which causes us to do bad thing as the value is cast into an integer and
        // overflow when expanding the bounds a few lines below.
        // Instead we are better off returning zero-sized metrics because this special
        // case is handled by the callers of this method.
        return GlyphMetrics {
            rasterized_left: 0,
            rasterized_width: 0,
            rasterized_height: 0,
            rasterized_ascent: 0,
            rasterized_descent: 0,
            advance: 0.0,
        };
    }

    let mut advance = CGSize { width: 0.0, height: 0.0 };
    unsafe {
        ct_font.get_advances_for_glyphs(kCTFontDefaultOrientation, &glyph, &mut advance, 1);
    }

    if bounds.size.width > 0.0 {
        bounds.size.width += extra_width;
    }
    if advance.width > 0.0 {
        advance.width += extra_width;
    }

    if let Some(transform) = transform {
        bounds = bounds.apply_transform(transform);
    }

    // First round out to pixel boundaries
    // CG Origin is bottom left
    let mut left = bounds.origin.x.floor() as i32;
    let mut bottom = bounds.origin.y.floor() as i32;
    let mut right = (bounds.origin.x + bounds.size.width + x_offset).ceil() as i32;
    let mut top = (bounds.origin.y + bounds.size.height + y_offset).ceil() as i32;

    // Expand the bounds by 1 pixel, to give CG room for anti-aliasing.
    // Note that this outset is to allow room for LCD smoothed glyphs. However, the correct outset
    // is not currently known, as CG dilates the outlines by some percentage.
    // This is taken from Skia.
    left -= 1;
    bottom -= 1;
    right += 1;
    top += 1;

    let width = right - left;
    let height = top - bottom;

    GlyphMetrics {
        rasterized_left: left,
        rasterized_width: width,
        rasterized_height: height,
        rasterized_ascent: top,
        rasterized_descent: -bottom,
        advance: advance.width as f32,
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
extern {
    static kCTFontVariationAxisIdentifierKey: CFStringRef;
    static kCTFontVariationAxisNameKey: CFStringRef;
    static kCTFontVariationAxisMinimumValueKey: CFStringRef;
    static kCTFontVariationAxisMaximumValueKey: CFStringRef;
    static kCTFontVariationAxisDefaultValueKey: CFStringRef;

    fn CTFontCopyVariationAxes(font: CTFontRef) -> CFArrayRef;

}

fn new_ct_font_with_variations(cg_font: &CGFont, size: f64, variations: &[FontVariation]) -> CTFont {
    unsafe {
        let ct_font = core_text::font::new_from_CGFont(cg_font, size);
        if variations.is_empty() {
            return ct_font;
        }
        let axes_ref = CTFontCopyVariationAxes(ct_font.as_concrete_TypeRef());
        if axes_ref.is_null() {
            return ct_font;
        }
        let axes: CFArray<CFDictionary> = TCFType::wrap_under_create_rule(axes_ref);
        let mut vals: Vec<(CFString, CFNumber)> = Vec::with_capacity(variations.len() as usize);
        for axis in axes.iter() {
            if !axis.instance_of::<CFDictionary>() {
                return ct_font;
            }
            let tag_val = match axis.find(kCTFontVariationAxisIdentifierKey as *const _) {
                Some(tag_ptr) => {
                    let tag: CFNumber = TCFType::wrap_under_get_rule(*tag_ptr as CFNumberRef);
                    if !tag.instance_of::<CFNumber>() {
                        return ct_font;
                    }
                    match tag.to_i64() {
                        Some(val) => val,
                        None => return ct_font,
                    }
                }
                None => return ct_font,
            };
            let mut val = match variations.iter().find(|variation| (variation.tag as i64) == tag_val) {
                Some(variation) => variation.value as f64,
                None => continue,
            };

            let name: CFString = match axis.find(kCTFontVariationAxisNameKey as *const _) {
                Some(name_ptr) => TCFType::wrap_under_get_rule(*name_ptr as CFStringRef),
                None => return ct_font,
            };
            if !name.instance_of::<CFString>() {
                return ct_font;
            }

            let min_val = match axis.find(kCTFontVariationAxisMinimumValueKey as *const _) {
                Some(min_ptr) => {
                    let min: CFNumber = TCFType::wrap_under_get_rule(*min_ptr as CFNumberRef);
                    if !min.instance_of::<CFNumber>() {
                        return ct_font;
                    }
                    match min.to_f64() {
                        Some(val) => val,
                        None => return ct_font,
                    }
                }
                None => return ct_font,
            };
            let max_val = match axis.find(kCTFontVariationAxisMaximumValueKey as *const _) {
                Some(max_ptr) => {
                    let max: CFNumber = TCFType::wrap_under_get_rule(*max_ptr as CFNumberRef);
                    if !max.instance_of::<CFNumber>() {
                        return ct_font;
                    }
                    match max.to_f64() {
                        Some(val) => val,
                        None => return ct_font,
                    }
                }
                None => return ct_font,
            };
            let def_val = match axis.find(kCTFontVariationAxisDefaultValueKey as *const _) {
                Some(def_ptr) => {
                    let def: CFNumber = TCFType::wrap_under_get_rule(*def_ptr as CFNumberRef);
                    if !def.instance_of::<CFNumber>() {
                        return ct_font;
                    }
                    match def.to_f64() {
                        Some(val) => val,
                        None => return ct_font,
                    }
                }
                None => return ct_font,
            };

            val = val.max(min_val).min(max_val);
            if val != def_val {
                vals.push((name, CFNumber::from(val)));
            }
        }
        if vals.is_empty() {
            return ct_font;
        }
        let vals_dict = CFDictionary::from_CFType_pairs(&vals);
        let cg_var_font = cg_font.create_copy_from_variations(&vals_dict).unwrap();
        core_text::font::new_from_CGFont_with_variations(&cg_var_font, size, &vals_dict)
    }

}
// We rely on Gecko to determine whether the font may have color glyphs to avoid
// needing to load the font ahead of time to query its symbolic traits.
fn is_bitmap_font(font: &FontInstance) -> bool {
    font.flags.contains(FontInstanceFlags::EMBEDDED_BITMAPS)
}

impl FontContext {
    pub fn distribute_across_threads() -> bool {
        true
    }

    pub fn new() -> FontContext {
        debug!("Test for subpixel AA support: {:?}", *FONT_SMOOTHING_MODE);

        // Force CG to use sRGB color space to gamma correct.
        let contrast = 0.0;
        let gamma = 0.0;

        FontContext {
            cg_fonts: FastHashMap::default(),
            ct_fonts: FastHashMap::default(),
            graphics_context: GraphicsContext::new(),
            gamma_lut: GammaLut::new(contrast, gamma, gamma),
        }
    }

    pub fn add_raw_font(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, index: u32) {
        if self.cg_fonts.contains_key(font_key) {
            return;
        }

        assert_eq!(index, 0);
        let data_provider = CGDataProvider::from_buffer(bytes); 
        let cg_font = match CGFont::from_data_provider(data_provider) { 
            Err(_) => return,
            Ok(cg_font) => cg_font,
        };
        self.cg_fonts.insert(*font_key, cg_font);
    }

    pub fn add_native_font(&mut self, font_key: &FontKey, native_font_handle: NativeFontHandle) {
        if self.cg_fonts.contains_key(font_key) {
            return;
        }

        // There's no way great way to go from a CGFont to a CTFontDescriptor
        // We could use the postscript name but that doesn't work for the
        // system UI fonts on newer macOS versions. Instead we create a CTFont
        // and use the descriptor for that. Normally we'd try to avoid new_from_CGFont
        // because that adds the CGFont to the descriptor cache which can keep the CGFont
        // around for a long time, but that should be ok for non-web (native) fonts.
        let cf_name = CFString::new(&native_font_handle.name);

        // For "hidden" system fonts, whose names start with a period,
        // we can't instantiate CTFonts via a descriptor. We're really
        // supposed to use CTFontCreateUIFontForLanguage, but for now
        // we just use the CGFont.
        let mut desc = if native_font_handle.name.starts_with('.') {
            let cg_font = match CGFont::from_name(&cf_name) {
                Ok(cg_font) => cg_font,
                Err(_) => {
                    // If for some reason we failed to load a font descriptor, then our
                    // only options are to either abort or substitute a fallback font.
                    // It is preferable to use a fallback font instead so that rendering
                    // can at least still proceed in some fashion without erroring.
                    // Lucida Grande is the fallback font in Gecko, so use that here.
                    CGFont::from_name(&CFString::from_static_string("Lucida Grande"))
                        .expect("couldn't find font with postscript name and couldn't load fallback font")
                }
            };
            core_text::font::new_from_CGFont(&cg_font, 0.).copy_descriptor()
        } else {
            core_text::font_descriptor::new_from_postscript_name(&cf_name)
        };

        // If the NativeFontHandle includes a file path, add this to the descriptor
        // to disambiguate cases where multiple installed fonts have the same psname.
        if native_font_handle.path.len() > 0 {
            let cf_path = CFString::new(&native_font_handle.path);
            let url_attribute = unsafe { CFString::wrap_under_get_rule(kCTFontURLAttribute) };
            let attrs = CFDictionary::from_CFType_pairs(&[
                (url_attribute, CFURL::from_file_system_path(cf_path, kCFURLPOSIXPathStyle, false)),
            ]);
            if let Ok(desc_with_path) = desc.create_copy_with_attributes(attrs.to_untyped()) {
                desc = desc_with_path;
            }
        }

        self.cg_fonts
            .insert(*font_key, CGFont::from_name(&cf_name).unwrap());
    }

    pub fn delete_font(&mut self, font_key: &FontKey) {
        if let Some(_) = self.cg_fonts.remove(font_key) {
            self.ct_fonts.retain(|k, _| k.0 != *font_key);
        }
    }

    pub fn delete_font_instance(&mut self, instance: &FontInstance) {
        // Remove the CoreText font corresponding to this instance.
        let size = FontSize::from_f64_px(instance.get_transformed_size());
        self.ct_fonts.remove(&(instance.font_key, size, instance.variations.clone()));
    }

    fn get_ct_font(
        &mut self,
        font_key: FontKey,
        size: f64,
        variations: &[FontVariation],
    ) -> Option<CTFont> {
        // Interacting with CoreText can create autorelease garbage.
        objc::rc::autoreleasepool(|| {
            match self.ct_fonts.entry((font_key, FontSize::from_f64_px(size), variations.to_vec())) {
                Entry::Occupied(entry) => Some((*entry.get()).clone()),
                Entry::Vacant(entry) => {
                    let cg_font = self.cg_fonts.get(&font_key)?;
                    let ct_font = new_ct_font_with_variations(cg_font, size, variations);
                    entry.insert(ct_font.clone());
                    Some(ct_font)
                }
            }
        })
    }

    pub fn get_glyph_index(&mut self, font_key: FontKey, ch: char) -> Option<u32> {
        let character = ch as u16;
        let mut glyph = 0;

        self.get_ct_font(font_key, 16.0, &[])
            .and_then(|ct_font| {
                unsafe {
                    let result = ct_font.get_glyphs_for_characters(&character, &mut glyph, 1);

                    if result {
                        Some(glyph as u32)
                    } else {
                        None
                    }
                }
            })
    }

    pub fn get_glyph_dimensions(
        &mut self,
        font: &FontInstance,
        key: &GlyphKey,
    ) -> Option<GlyphDimensions> {
        let (x_scale, y_scale) = font.transform.compute_scale().unwrap_or((1.0, 1.0));
        let size = font.size.to_f64_px() * y_scale;
        self.get_ct_font(font.font_key, size, &font.variations)
            .and_then(|ct_font| {
                let glyph = key.index() as CGGlyph;
                let bitmap = is_bitmap_font(font);
                let (mut shape, (x_offset, y_offset)) = if bitmap {
                    (FontTransform::identity(), (0.0, 0.0))
                } else {
                    (font.transform.invert_scale(y_scale, y_scale), font.get_subpx_offset(key))
                };
                if font.flags.contains(FontInstanceFlags::FLIP_X) {
                    shape = shape.flip_x();
                }
                if font.flags.contains(FontInstanceFlags::FLIP_Y) {
                    shape = shape.flip_y();
                }
                if font.flags.contains(FontInstanceFlags::TRANSPOSE) {
                    shape = shape.swap_xy();
                }
                let (mut tx, mut ty) = (0.0, 0.0);
                if font.synthetic_italics.is_enabled() {
                    let (shape_, (tx_, ty_)) = font.synthesize_italics(shape, size);
                    shape = shape_;
                    tx = tx_;
                    ty = ty_;
                }
                let transform = if !shape.is_identity() || (tx, ty) != (0.0, 0.0) {
                    Some(CGAffineTransform {
                        a: shape.scale_x as f64,
                        b: -shape.skew_y as f64,
                        c: -shape.skew_x as f64,
                        d: shape.scale_y as f64,
                        tx: tx,
                        ty: -ty,
                    })
                } else {
                    None
                };
                let (strike_scale, pixel_step) = if bitmap {
                    (y_scale, 1.0)
                } else {
                    (x_scale, y_scale / x_scale)
                };
                let extra_strikes = font.get_extra_strikes(
                    FontInstanceFlags::SYNTHETIC_BOLD | FontInstanceFlags::MULTISTRIKE_BOLD,
                    strike_scale,
                );
                let metrics = get_glyph_metrics(
                    &ct_font,
                    transform.as_ref(),
                    glyph,
                    x_offset,
                    y_offset,
                    extra_strikes as f64 * pixel_step,
                );
                if metrics.rasterized_width == 0 || metrics.rasterized_height == 0 {
                    None
                } else {
                    Some(GlyphDimensions {
                        left: metrics.rasterized_left,
                        top: metrics.rasterized_ascent,
                        width: metrics.rasterized_width,
                        height: metrics.rasterized_height,
                        advance: metrics.advance,
                    })
                }
            })
    }

    // Assumes the pixels here are linear values from CG
    fn gamma_correct_pixels(
        &self,
        pixels: &mut Vec<u8>,
        render_mode: FontRenderMode,
        color: ColorU,
    ) {
        let ColorU {r, g, b, a} = color;
        let smooth_color = match *FONT_SMOOTHING_MODE {
            // Use Skia's gamma approximation for subpixel smoothing of 3/4.
            Some(FontRenderMode::Subpixel) => ColorU::new(r - r / 4, g - g / 4, b - b / 4, a),
            // Use Skia's gamma approximation for grayscale smoothing of 1/2.
            Some(FontRenderMode::Alpha) => ColorU::new(r / 2, g / 2, b / 2, a),
            _ => color,
        };

        // Then convert back to gamma corrected values.
        match render_mode {
            FontRenderMode::Alpha => {
                self.gamma_lut.preblend_grayscale(pixels, smooth_color);
            }
            FontRenderMode::Subpixel => {
                self.gamma_lut.preblend(pixels, smooth_color);
            }
            _ => {} // Again, give mono untouched since only the alpha matters.
        }
    }

    #[allow(dead_code)]
    fn print_glyph_data(&mut self, data: &[u8], width: usize, height: usize) {
        // Rust doesn't have step_by support on stable :(
        debug!("Width is: {:?} height: {:?}", width, height);
        for i in 0 .. height {
            let current_height = i * width * 4;

            for pixel in data[current_height .. current_height + (width * 4)].chunks(4) {
                let b = pixel[0];
                let g = pixel[1];
                let r = pixel[2];
                let a = pixel[3];
                debug!("({}, {}, {}, {}) ", r, g, b, a);
            }
        }
    }

    pub fn prepare_font(font: &mut FontInstance) {
        if is_bitmap_font(font) {
            // Render mode is ignored for bitmap fonts. Also, avoid normalizing the color
            // in case CoreText needs the current color for rendering glyph color layers.
            font.render_mode = FontRenderMode::Mono;
            font.disable_subpixel_position();
            return;
        }
        // Sanitize the render mode for font smoothing. If font smoothing is supported,
        // then we just need to ensure the render mode is limited to what is supported.
        // If font smoothing is actually disabled, then we need to fall back to grayscale.
        if font.flags.contains(FontInstanceFlags::FONT_SMOOTHING) ||
            font.render_mode == FontRenderMode::Subpixel {
            match *FONT_SMOOTHING_MODE {
                Some(mode) => {
                    font.render_mode = font.render_mode.limit_by(mode);
                    font.flags.insert(FontInstanceFlags::FONT_SMOOTHING);
                }
                None => {
                    font.render_mode = font.render_mode.limit_by(FontRenderMode::Alpha);
                    font.flags.remove(FontInstanceFlags::FONT_SMOOTHING);
                }
            }
        }
        match font.render_mode {
            FontRenderMode::Mono => {
                // In mono mode the color of the font is irrelevant.
                font.color = ColorU::new(255, 255, 255, 255);
                // Subpixel positioning is disabled in mono mode.
                font.disable_subpixel_position();
            }
            FontRenderMode::Alpha => {
                font.color = if font.flags.contains(FontInstanceFlags::FONT_SMOOTHING) {
                    font.color.luminance_color().quantize()
                } else {
                    ColorU::new(255, 255, 255, 255)
                };
            }
            FontRenderMode::Subpixel => {
                font.color = font.color.quantize();
            }
        }
    }

    pub fn begin_rasterize(_font: &FontInstance) {
    }

    pub fn end_rasterize(_font: &FontInstance) {
    }

    pub fn rasterize_glyph(&mut self, font: &FontInstance, key: &GlyphKey) -> GlyphRasterResult {
        objc::rc::autoreleasepool(|| {
        let (x_scale, y_scale) = font.transform.compute_scale().unwrap_or((1.0, 1.0));
        let size = font.size.to_f64_px() * y_scale;
        let ct_font =
            self.get_ct_font(font.font_key, size, &font.variations).ok_or(GlyphRasterError::LoadFailed)?;
        let glyph_type = if is_bitmap_font(font) {
            GlyphType::Bitmap
        } else {
            GlyphType::Vector
        };

        let (mut shape, (x_offset, y_offset)) = match glyph_type {
            GlyphType::Bitmap => (FontTransform::identity(), (0.0, 0.0)),
            GlyphType::Vector => {
                (font.transform.invert_scale(y_scale, y_scale), font.get_subpx_offset(key))
            }
        };
        if font.flags.contains(FontInstanceFlags::FLIP_X) {
            shape = shape.flip_x();
        }
        if font.flags.contains(FontInstanceFlags::FLIP_Y) {
            shape = shape.flip_y();
        }
        if font.flags.contains(FontInstanceFlags::TRANSPOSE) {
            shape = shape.swap_xy();
        }
        let (mut tx, mut ty) = (0.0, 0.0);
        if font.synthetic_italics.is_enabled() {
            let (shape_, (tx_, ty_)) = font.synthesize_italics(shape, size);
            shape = shape_;
            tx = tx_;
            ty = ty_;
        }
        let transform = if !shape.is_identity() || (tx, ty) != (0.0, 0.0) {
            Some(CGAffineTransform {
                a: shape.scale_x as f64,
                b: -shape.skew_y as f64,
                c: -shape.skew_x as f64,
                d: shape.scale_y as f64,
                tx: tx,
                ty: -ty,
            })
        } else {
            None
        };

        let glyph = key.index() as CGGlyph;
        let (strike_scale, pixel_step) = if glyph_type == GlyphType::Bitmap {
            (y_scale, 1.0)
        } else {
            (x_scale, y_scale / x_scale)
        };
        let extra_strikes = font.get_extra_strikes(
            FontInstanceFlags::SYNTHETIC_BOLD | FontInstanceFlags::MULTISTRIKE_BOLD,
            strike_scale,
        );
        let metrics = get_glyph_metrics(
            &ct_font,
            transform.as_ref(),
            glyph,
            x_offset,
            y_offset,
            extra_strikes as f64 * pixel_step,
        );
        if metrics.rasterized_width == 0 || metrics.rasterized_height == 0 {
            return Err(GlyphRasterError::LoadFailed);
        }

        let raster_size = Size2D::new(
            metrics.rasterized_width as u32,
            metrics.rasterized_height as u32
        );

        // If the font render mode is Alpha, we support two different ways to
        // compute the grayscale mask, depending on the value of the platform
        // options' font_smoothing flag:
        //  - Alpha + smoothing:
        //    We will recover a grayscale mask from a subpixel rasterization, in
        //    such a way that the result looks as close to subpixel text
        //    blending as we can make it. This involves gamma correction,
        //    luminance computations and preblending based on the text color,
        //    just like with the Subpixel render mode.
        //  - Alpha without smoothing:
        //    We will ask CoreGraphics to rasterize the text with font_smoothing
        //    off. This will cause it to use grayscale anti-aliasing with
        //    comparatively thin text. This method of text rendering is not
        //    gamma-aware.
        //
        // For subpixel rasterization, starting with macOS 10.11, CoreGraphics
        // uses different glyph dilation based on the text color. Bright text
        // uses less font dilation (looks thinner) than dark text.
        // As a consequence, when we ask CG to rasterize with subpixel AA, we
        // will render white-on-black text as opposed to black-on-white text if
        // the text color brightness exceeds a certain threshold. This applies
        // to both the Subpixel and the "Alpha + smoothing" modes, but not to
        // the "Alpha without smoothing" and Mono modes.
        //
        // Fonts with color glyphs may, depending on the state within per-glyph
        // table data, require the current font color to determine the output
        // color. For such fonts we must thus supply the current font color just
        // in case it is necessary.
        let use_font_smoothing = font.flags.contains(FontInstanceFlags::FONT_SMOOTHING);
        let (antialias, smooth, text_color, bg_color) = match glyph_type {
            GlyphType::Bitmap => (true, false, ColorF::from(font.color), ColorF::TRANSPARENT),
            GlyphType::Vector => {
                match (font.render_mode, use_font_smoothing) {
                    (FontRenderMode::Subpixel, _) |
                    (FontRenderMode::Alpha, true) => (true, true, ColorF::BLACK, ColorF::WHITE),
                    (FontRenderMode::Alpha, false) => (true, false, ColorF::BLACK, ColorF::WHITE),
                    (FontRenderMode::Mono, _) => (false, false, ColorF::BLACK, ColorF::WHITE),
                }
            }
        };

        {
            let cg_context = self.graphics_context.get_context(&raster_size, glyph_type);

            // These are always true in Gecko, even for non-AA fonts
            cg_context.set_allows_font_subpixel_positioning(true);
            cg_context.set_should_subpixel_position_fonts(true);

            // Don't quantize because we're doing it already.
            cg_context.set_allows_font_subpixel_quantization(false);
            cg_context.set_should_subpixel_quantize_fonts(false);

            cg_context.set_should_smooth_fonts(smooth);
            cg_context.set_should_antialias(antialias);

            // Fill the background. This could be opaque white, opaque black, or
            // transparency.
            cg_context.set_rgb_fill_color(
                bg_color.r.into(),
                bg_color.g.into(),
                bg_color.b.into(),
                bg_color.a.into(),
            );
            let rect = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: metrics.rasterized_width as f64,
                    height: metrics.rasterized_height as f64,
                },
            };

            // Make sure we use the Copy blend mode, or else we'll get the Porter-Duff OVER
            // operator, which can't clear to the transparent color!
            cg_context.set_blend_mode(CGBlendMode::Copy);
            cg_context.fill_rect(rect);
            cg_context.set_blend_mode(CGBlendMode::Normal);

            // Set the text color and draw the glyphs.
            cg_context.set_rgb_fill_color(
                text_color.r.into(),
                text_color.g.into(),
                text_color.b.into(),
                1.0,
            );
            cg_context.set_text_drawing_mode(CGTextDrawingMode::CGTextFill);

            // CG Origin is bottom left, WR is top left. Need -y offset
            let mut draw_origin = CGPoint {
                x: -metrics.rasterized_left as f64 + x_offset + tx,
                y: metrics.rasterized_descent as f64 - y_offset - ty,
            };

            if let Some(transform) = transform {
                cg_context.set_text_matrix(&transform);

                draw_origin = draw_origin.apply_transform(&transform.invert());
            } else {
                // Make sure to reset this because some previous glyph rasterization might have
                // changed it.
                cg_context.set_text_matrix(&CG_AFFINE_TRANSFORM_IDENTITY);
            }

            ct_font.draw_glyphs(&[glyph], &[draw_origin], cg_context.clone());

            // We'd like to render all the strikes in a single ct_font.draw_glyphs call,
            // passing an array of glyph IDs and an array of origins, but unfortunately
            // with some fonts, Core Text may inappropriately pixel-snap the rasterization,
            // such that the strikes overprint instead of being offset. Rendering the
            // strikes with individual draw_glyphs calls avoids this.
            // (See https://bugzilla.mozilla.org/show_bug.cgi?id=1633397 for details.)
            for i in 1 ..= extra_strikes {
                let origin = CGPoint {
                    x: draw_origin.x + i as f64 * pixel_step,
                    y: draw_origin.y,
                };
                ct_font.draw_glyphs(&[glyph], &[origin], cg_context.clone());
            }
        }

        let mut rasterized_pixels = self.graphics_context
                                        .get_rasterized_pixels(&raster_size, glyph_type);

        if glyph_type == GlyphType::Vector {
            // We rendered text into an opaque surface. The code below needs to
            // ignore the current value of each pixel's alpha channel. But it's
            // allowed to write to the alpha channel, because we're done calling
            // CG functions now.

            if smooth {
                // Convert to linear space for subpixel AA.
                // We explicitly do not do this for grayscale AA ("Alpha without
                // smoothing" or Mono) because those rendering modes are not
                // gamma-aware in CoreGraphics.
                self.gamma_lut.coregraphics_convert_to_linear(
                    &mut rasterized_pixels,
                );
            }

            for pixel in rasterized_pixels.chunks_mut(4) {
                pixel[0] = 255 - pixel[0];
                pixel[1] = 255 - pixel[1];
                pixel[2] = 255 - pixel[2];

                // Set alpha to the value of the green channel. For grayscale
                // text, all three channels have the same value anyway.
                // For subpixel text, the mask's alpha only makes a difference
                // when computing the destination alpha on destination pixels
                // that are not completely opaque. Picking an alpha value
                // that's somehow based on the mask at least ensures that text
                // blending doesn't modify the destination alpha on pixels where
                // the mask is entirely zero.
                pixel[3] = pixel[1];
            }

            if smooth {
                // Convert back from linear space into device space, and perform
                // some "preblending" based on the text color.
                // In Alpha + smoothing mode, this will also convert subpixel AA
                // into grayscale AA.
                self.gamma_correct_pixels(
                    &mut rasterized_pixels,
                    font.render_mode,
                    font.color,
                );
            }
        }

        Ok(RasterizedGlyph {
            left: metrics.rasterized_left as f32,
            top: metrics.rasterized_ascent as f32,
            width: metrics.rasterized_width,
            height: metrics.rasterized_height,
            scale: match glyph_type {
                GlyphType::Bitmap => y_scale.recip() as f32,
                GlyphType::Vector => 1.0,
            },
            format: match glyph_type {
                GlyphType::Bitmap => GlyphFormat::ColorBitmap,
                GlyphType::Vector => font.get_glyph_format(),
            },
            bytes: rasterized_pixels,
        })})
    }
}

// Avoids taking locks by recycling Core Graphics contexts.
#[allow(dead_code)]
struct GraphicsContext {
    vector_context: CGContext,
    vector_context_size: Size2D<u32>,
    bitmap_context: CGContext,
    bitmap_context_size: Size2D<u32>,
}

impl GraphicsContext {
    fn new() -> GraphicsContext {
        let size = Size2D::new(INITIAL_CG_CONTEXT_SIDE_LENGTH, INITIAL_CG_CONTEXT_SIDE_LENGTH);
        GraphicsContext {
            vector_context: GraphicsContext::create_cg_context(&size, GlyphType::Vector),
            vector_context_size: size,
            bitmap_context: GraphicsContext::create_cg_context(&size, GlyphType::Bitmap),
            bitmap_context_size: size,
        }
    }

    #[allow(dead_code)]
    fn get_context(&mut self, size: &Size2D<u32>, glyph_type: GlyphType)
                   -> &mut CGContext {
        let (cached_context, cached_size) = match glyph_type {
            GlyphType::Vector => {
                (&mut self.vector_context, &mut self.vector_context_size)
            }
            GlyphType::Bitmap => {
                (&mut self.bitmap_context, &mut self.bitmap_context_size)
            }
        };
        let rounded_size = Size2D::new(size.width.next_power_of_two(),
                                       size.height.next_power_of_two());
        if rounded_size.width > cached_size.width || rounded_size.height > cached_size.height {
            *cached_size = Size2D::new(u32::max(cached_size.width, rounded_size.width),
                                       u32::max(cached_size.height, rounded_size.height));
            *cached_context = GraphicsContext::create_cg_context(cached_size, glyph_type);
        }
        cached_context
    }

    #[allow(dead_code)]
    fn get_rasterized_pixels(&mut self, size: &Size2D<u32>, glyph_type: GlyphType)
                             -> Vec<u8> {
        let (cached_context, cached_size) = match glyph_type {
            GlyphType::Vector => (&mut self.vector_context, &self.vector_context_size),
            GlyphType::Bitmap => (&mut self.bitmap_context, &self.bitmap_context_size),
        };
        let cached_data = cached_context.data();
        let cached_stride = cached_size.width as usize * 4;

        let result_len = size.width as usize * size.height as usize * 4;
        let mut result = Vec::with_capacity(result_len);
        for y in (cached_size.height - size.height)..cached_size.height {
            let cached_start = y as usize * cached_stride;
            let cached_end = cached_start + size.width as usize * 4;
            result.extend_from_slice(&cached_data[cached_start..cached_end]);
        }
        debug_assert_eq!(result.len(), result_len);
        result
    }

    fn create_cg_context(size: &Size2D<u32>, glyph_type: GlyphType) -> CGContext {
        // The result of rasterization, in all render modes, is going to be a
        // BGRA surface with white text on transparency using premultiplied
        // alpha. For subpixel text, the RGB values will be the mask value for
        // the individual components. For bitmap glyphs, the RGB values will be
        // the (premultiplied) color of the pixel. For Alpha and Mono, each
        // pixel will have R==G==B==A at the end of this function.
        // We access the color channels in little-endian order.
        // The CGContext will create and own our pixel buffer.
        // In the non-Bitmap cases, we will ask CoreGraphics to draw text onto
        // an opaque background. In order to hit the most efficient path in CG
        // for this, we will tell CG that the CGContext is opaque, by passing
        // an "[...]AlphaNone[...]" context flag. This creates a slight
        // contradiction to the way we use the buffer after CG is done with it,
        // because we will convert it into text-on-transparency. But that's ok;
        // we still get four bytes per pixel and CG won't mess with the alpha
        // channel after we've stopped calling CG functions. We just need to
        // make sure that we don't look at the alpha values of the pixels that
        // we get from CG, and compute our own alpha value only from RGB.
        // Note that CG requires kCGBitmapByteOrder32Little in order to do
        // subpixel AA at all (which we need it to do in both Subpixel and
        // Alpha+smoothing mode). But little-endian is what we want anyway, so
        // this works out nicely.
        let color_type = match glyph_type {
            GlyphType::Vector => kCGImageAlphaNoneSkipFirst,
            GlyphType::Bitmap => kCGImageAlphaPremultipliedFirst,
        };

        CGContext::create_bitmap_context(None,
                                         size.width as usize,
                                         size.height as usize,
                                         8,
                                         size.width as usize * 4,
                                         &CGColorSpace::create_device_rgb(),
                                         kCGBitmapByteOrder32Little | color_type)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum GlyphType {
    Vector,
    Bitmap,
}

