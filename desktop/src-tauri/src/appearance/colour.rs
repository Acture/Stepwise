//! Colours as the theme mapper handles them: read from a theme's hex, laid over a ground,
//! mixed, and weighed by WCAG contrast. A colour is kept in whole 8-bit channels, exactly as
//! the page will be told it, so a contrast judged here is the contrast the page draws.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
	r: u8,
	g: u8,
	b: u8,
}

/// A colour as a theme writes it, which may be translucent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
	rgb: Rgb,
	alpha: f64,
}

impl Rgba {
	/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`: the forms a VS Code theme accepts.
	pub fn hex(text: &str) -> Option<Self> {
		let digits: &str = text.strip_prefix('#')?;
		if !digits.bytes().all(|byte: u8| byte.is_ascii_hexdigit()) {
			return None;
		}
		let channels: Vec<u8> = match digits.len() {
			3 | 4 => digits
				.chars()
				.map(|digit: char| digit.to_digit(16).map_or(0, |value: u32| value as u8 * 17))
				.collect(),
			6 | 8 => (0..digits.len())
				.step_by(2)
				.map(|at: usize| u8::from_str_radix(&digits[at..at + 2], 16).unwrap_or(0))
				.collect(),
			_ => return None,
		};
		Some(Self {
			rgb: Rgb {
				r: channels[0],
				g: channels[1],
				b: channels[2],
			},
			alpha: channels
				.get(3)
				.map_or(1.0, |alpha: &u8| f64::from(*alpha) / 255.0),
		})
	}

	/// This colour at `opacity` of its own.
	pub fn faded(self, opacity: f64) -> Self {
		Self {
			alpha: self.alpha * opacity,
			..self
		}
	}

	/// This colour laid over an opaque ground.
	pub fn over(self, ground: Rgb) -> Rgb {
		ground.mix(self.rgb, self.alpha)
	}
}

impl Rgb {
	pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
	pub const WHITE: Self = Self {
		r: 255,
		g: 255,
		b: 255,
	};

	/// Moves `share` of the way from this colour to `toward`, channel by channel.
	pub fn mix(self, toward: Self, share: f64) -> Self {
		let channel = |from: u8, to: u8| -> u8 {
			(f64::from(from) + (f64::from(to) - f64::from(from)) * share.clamp(0.0, 1.0)).round()
				as u8
		};
		Self {
			r: channel(self.r, toward.r),
			g: channel(self.g, toward.g),
			b: channel(self.b, toward.b),
		}
	}

	/// WCAG relative luminance.
	pub fn luminance(self) -> f64 {
		let linear = |channel: u8| -> f64 {
			let value: f64 = f64::from(channel) / 255.0;
			if value <= 0.04045 {
				value / 12.92
			} else {
				((value + 0.055) / 1.055).powf(2.4)
			}
		};
		0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
	}

	/// WCAG contrast ratio, from 1 to 21.
	pub fn contrast(self, other: Self) -> f64 {
		let (a, b): (f64, f64) = (self.luminance(), other.luminance());
		(a.max(b) + 0.05) / (a.min(b) + 0.05)
	}

	/// How far from grey, from 0 to 1: the spread between the strongest and weakest channel.
	pub fn chroma(self) -> f64 {
		let high: u8 = self.r.max(self.g).max(self.b);
		let low: u8 = self.r.min(self.g).min(self.b);
		f64::from(high - low) / 255.0
	}

	/// How far apart two colours are, from 0 to 1: the widest gap in any one channel.
	pub fn apart(self, other: Self) -> f64 {
		let gap = |a: u8, b: u8| -> u8 { a.abs_diff(b) };
		f64::from(
			gap(self.r, other.r)
				.max(gap(self.g, other.g))
				.max(gap(self.b, other.b)),
		) / 255.0
	}

	/// As CSS: `#rrggbb`.
	pub fn css(self) -> String {
		format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
	}

	/// As CSS with an alpha: `rgba(r, g, b, a)`.
	pub fn css_alpha(self, alpha: f64) -> String {
		format!("rgba({}, {}, {}, {alpha})", self.r, self.g, self.b)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_hex_form_a_theme_may_use_is_read() {
		let rgb = |r: u8, g: u8, b: u8| -> Rgb { Rgb { r, g, b } };
		assert_eq!(Rgba::hex("#fff").map(|c: Rgba| c.rgb), Some(Rgb::WHITE));
		assert_eq!(
			Rgba::hex("#1E1E1E").map(|c: Rgba| c.rgb),
			Some(rgb(30, 30, 30))
		);
		assert_eq!(Rgba::hex("#ccc3").map(|c: Rgba| c.alpha), Some(0.2));
		assert_eq!(
			Rgba::hex("#ADD6FF26").map(|c: Rgba| (c.rgb, (c.alpha * 255.0).round())),
			Some((rgb(173, 214, 255), 38.0))
		);
		for text in ["", "fff", "#ff", "#fffff", "#ggg", "red", "#ff00ff0"] {
			assert_eq!(Rgba::hex(text), None, "{text}");
		}
	}

	#[test]
	fn translucency_is_laid_over_its_ground() {
		let half: Rgba = Rgba::hex("#ffffff80").expect("a colour");
		assert_eq!(half.over(Rgb::BLACK).css(), "#808080");
		assert_eq!(
			Rgba::hex("#123456")
				.expect("a colour")
				.over(Rgb::WHITE)
				.css(),
			"#123456"
		);
	}

	#[test]
	fn contrast_follows_wcag() {
		assert_eq!(Rgb::BLACK.contrast(Rgb::WHITE), 21.0);
		assert_eq!(Rgb::WHITE.contrast(Rgb::WHITE), 1.0);
		let grey: Rgb = Rgb {
			r: 118,
			g: 118,
			b: 118,
		};
		assert!((grey.contrast(Rgb::WHITE) - 4.54).abs() < 0.01);
	}
}
