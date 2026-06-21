// --- CSS value types ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Auto,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Display {
    Inline,
    Block,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    Table,
    InlineTable,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableRow,
    TableColumnGroup,
    TableColumn,
    TableCell,
    TableCaption,
    ListItem,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderStyle {
    None,
    Hidden,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Float {
    None,
    Left,
    Right,
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhiteSpace {
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    Ltr,
    Rtl,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clear {
    None,
    Left,
    Right,
    Both,
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalAlign {
    Baseline,
    Top,
    Bottom,
    Middle,
    TextTop,
    TextBottom,
    Sub,
    Super,
    Length(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextDecorationLine {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
    pub blink: bool,
}

impl TextDecorationLine {
    pub fn none() -> Self {
        TextDecorationLine { underline: false, overline: false, line_through: false, blink: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundImage {
    None,
    Gradient { from: Color, to: Color, vertical: bool },
    Url(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

// --- Computed values for a styled element ---

#[derive(Clone, Debug)]
pub struct ComputedValues {
    pub display: Display,
    pub position: Position,
    pub width: Length,
    pub height: Length,
    pub margin_top: Length,
    pub margin_right: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,
    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,
    pub color: Color,
    pub background_color: Color,
    pub font_size: f32,
    pub z_index: i32,

    // Border
    pub border_top_width: f32,
    pub border_right_width: f32,
    pub border_bottom_width: f32,
    pub border_left_width: f32,
    pub border_top_style: BorderStyle,
    pub border_right_style: BorderStyle,
    pub border_bottom_style: BorderStyle,
    pub border_left_style: BorderStyle,
    pub border_top_color: Color,
    pub border_right_color: Color,
    pub border_bottom_color: Color,
    pub border_left_color: Color,
    pub border_top_left_radius: f32,
    pub border_top_right_radius: f32,
    pub border_bottom_right_radius: f32,
    pub border_bottom_left_radius: f32,

    // Effects
    pub opacity: f32,
    pub box_shadow: Vec<BoxShadow>,

    // Layout
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub float: Float,
    pub clear: Clear,
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,

    // Background
    pub background_image: Vec<BackgroundImage>,

    // Font & Text
    pub line_height: f32,
    pub font_family: String,
    pub white_space: WhiteSpace,
    pub direction: Direction,
    pub text_decoration_line: TextDecorationLine,
    pub text_decoration_color: Color,
    pub text_decoration_style: BorderStyle,
    pub text_align: TextAlign,
    pub vertical_align: VerticalAlign,

    // Sizing
    pub min_width: Length,
    pub max_width: Length,
    pub min_height: Length,
    pub max_height: Length,
    pub box_sizing: BoxSizing,

    // Visibility
    pub visibility: Visibility,

    // Outline
    pub outline_width: f32,
    pub outline_style: BorderStyle,
    pub outline_color: Color,
}

impl Default for ComputedValues {
    fn default() -> Self {
        ComputedValues {
            display: Display::Block,
            position: Position::Static,
            width: Length::Auto,
            height: Length::Auto,
            margin_top: Length::Px(0.0),
            margin_right: Length::Px(0.0),
            margin_bottom: Length::Px(0.0),
            margin_left: Length::Px(0.0),
            padding_top: Length::Px(0.0),
            padding_right: Length::Px(0.0),
            padding_bottom: Length::Px(0.0),
            padding_left: Length::Px(0.0),
            color: Color(0, 0, 0, 255),
            background_color: Color(255, 255, 255, 255),
            font_size: 16.0,
            z_index: 0,

            border_top_width: 0.0,
            border_right_width: 0.0,
            border_bottom_width: 0.0,
            border_left_width: 0.0,
            border_top_style: BorderStyle::None,
            border_right_style: BorderStyle::None,
            border_bottom_style: BorderStyle::None,
            border_left_style: BorderStyle::None,
            border_top_color: Color(0, 0, 0, 255),
            border_right_color: Color(0, 0, 0, 255),
            border_bottom_color: Color(0, 0, 0, 255),
            border_left_color: Color(0, 0, 0, 255),
            border_top_left_radius: 0.0,
            border_top_right_radius: 0.0,
            border_bottom_right_radius: 0.0,
            border_bottom_left_radius: 0.0,

            opacity: 1.0,
            box_shadow: vec![],

            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            float: Float::None,
            clear: Clear::None,
            top: Length::Auto,
            right: Length::Auto,
            bottom: Length::Auto,
            left: Length::Auto,

            background_image: vec![],

            line_height: 19.2,
            font_family: String::from("sans-serif"),
            white_space: WhiteSpace::Normal,
            direction: Direction::Ltr,
            text_decoration_line: TextDecorationLine::none(),
            text_decoration_color: Color(0, 0, 0, 255),
            text_decoration_style: BorderStyle::Solid,
            text_align: TextAlign::Start,
            vertical_align: VerticalAlign::Baseline,

            min_width: Length::Auto,
            max_width: Length::Auto,
            min_height: Length::Auto,
            max_height: Length::Auto,
            box_sizing: BoxSizing::ContentBox,

            visibility: Visibility::Visible,

            outline_width: 0.0,
            outline_style: BorderStyle::None,
            outline_color: Color(0, 0, 0, 255),
        }
    }
}

impl ComputedValues {
    pub fn inherit(parent: &ComputedValues) -> Self {
        let mut cv = ComputedValues::default();
        cv.color = parent.color.clone();
        cv.font_size = parent.font_size;
        cv.line_height = parent.line_height;
        cv.font_family = parent.font_family.clone();
        cv.white_space = parent.white_space;
        cv.direction = parent.direction;
        cv.text_align = parent.text_align;
        cv.visibility = parent.visibility;
        cv
    }

}
