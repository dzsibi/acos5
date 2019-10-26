/*
 * strings.c: default UI strings
 *
 * Copyright (C) 2017 Frank Morgner <frankmorgner@gmail.com>
 * Copyright (C) 2019-  for the binding: Carsten Blüggel <bluecars@posteo.eu>
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the Free Software
 * Foundation, 51 Franklin Street, Fifth Floor  Boston, MA 02110-1335  USA
 */

use std::os::raw::c_char;

use crate::types::sc_atr;
use crate::opensc::sc_context;
use crate::pkcs15::sc_pkcs15_card;

#[allow(non_camel_case_types)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum ui_str {
    MD_PINPAD_DLG_TITLE,
    MD_PINPAD_DLG_MAIN,
    MD_PINPAD_DLG_CONTENT_USER,
    MD_PINPAD_DLG_CONTENT_ADMIN,
    #[cfg(v0_18_0)]
    MD_PINPAD_DLG_CONTENT_CANCEL,
    MD_PINPAD_DLG_EXPANDED,
    #[cfg(v0_18_0)]
    MD_PINPAD_DLG_EXPANDED_CANCEL,
    MD_PINPAD_DLG_CONTROL_COLLAPSED,
    MD_PINPAD_DLG_CONTROL_EXPANDED,
    MD_PINPAD_DLG_ICON,
    MD_PINPAD_DLG_CANCEL,
    NOTIFY_CARD_INSERTED,
    NOTIFY_CARD_INSERTED_TEXT,
    NOTIFY_CARD_REMOVED,
    NOTIFY_CARD_REMOVED_TEXT,
    NOTIFY_PIN_GOOD,
    NOTIFY_PIN_GOOD_TEXT,
    NOTIFY_PIN_BAD,
    NOTIFY_PIN_BAD_TEXT,
    MD_PINPAD_DLG_CONTENT_USER_SIGN,
    NOTIFY_EXIT,
    MD_PINPAD_DLG_VERIFICATION,
}

extern "C" {
pub fn ui_get_str(ctx: *mut sc_context, atr: *mut sc_atr,
    p15card: *mut sc_pkcs15_card, id: ui_str) -> *const c_char;
}
