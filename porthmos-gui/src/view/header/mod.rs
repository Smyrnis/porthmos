use iced::widget::{container, mouse_area, row, space};
use iced::{Alignment, Element, Length, alignment};
use porthmos_core::listing::{SortKey, SortOrder};

use super::file_list::{Columns, EDGE_WIDTH, ICON_SLOT, MODIFIED_WIDTH, OWNER_WIDTH, PADDING, SIZE_WIDTH};
use super::{filled, label};
use crate::app::{Message, PanelSide};
use crate::geometry::COLUMN_HEADER_HEIGHT;
use crate::panel::Panel;
use crate::theme::Palette;

fn arrow(panel: &Panel, key: SortKey) -> &'static str {
    let spec = panel.sort_spec();
    match (spec.key == key, spec.order) {
        (false, _) => "",
        (true, SortOrder::Ascending) => " ▲",
        (true, SortOrder::Descending) => " ▼",
    }
}

fn sortable<'a>(
    side: PanelSide, panel: &Panel, key: SortKey, title: &str, width: Length, align: alignment::Horizontal,
    palette: &Palette,
) -> Element<'a, Message> {
    let sorted = panel.sort_spec().key == key;
    let color = if sorted { palette.fg_bright } else { palette.fg_dim };
    let title = label(format!("{title}{}", arrow(panel, key)), color).width(width).align_x(align);
    mouse_area(title).on_press(Message::HeaderClicked(side, key)).into()
}

fn plain<'a>(title: &str, width: f32, palette: &Palette) -> Element<'a, Message> {
    label(title.to_string(), palette.fg_dim).width(Length::Fixed(width)).into()
}

pub fn columns<'a>(side: PanelSide, panel: &Panel, shown: Columns, palette: &Palette) -> Element<'a, Message> {
    let mut line = row![
        space().width(Length::Fixed(EDGE_WIDTH + ICON_SLOT)),
        sortable(side, panel, SortKey::Name, "Name", Length::Fill, alignment::Horizontal::Left, palette),
    ]
    .align_y(Alignment::Center);
    if shown.size {
        let right = alignment::Horizontal::Right;
        line = line.push(sortable(side, panel, SortKey::Size, "Size", Length::Fixed(SIZE_WIDTH), right, palette));
    }
    if shown.modified {
        line = line.push(plain("  Modified", MODIFIED_WIDTH, palette));
    }
    if shown.owner {
        line = line.push(plain("  Owner", OWNER_WIDTH, palette));
    }
    container(line.push(space().width(Length::Fixed(PADDING))))
        .width(Length::Fill)
        .height(Length::Fixed(COLUMN_HEADER_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .style(filled(palette.bg))
        .into()
}
