/*
* no_cdecl.rs: Driver 'acos5_64' - Miscellaneous functions referring to sc_path or sc_path.value
 *
 * Copyright (C) 2019  Carsten Blüggel <bluecars@posteo.eu>
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

//extern crate bitintr;
//use bitintr::Popcnt;

//#![feature(const_fn)]

use std::os::raw::{c_int, c_void, c_uint, c_uchar, c_char, c_ulong};
use std::ffi::{/*CString,*/ CStr};
use std::fs;//::{read/*, write*/};
use std::ptr::copy_nonoverlapping;

use opensc_sys::opensc::{sc_card, sc_pin_cmd_data, sc_security_env,
                         sc_transmit_apdu, sc_bytes2apdu_wrapper, sc_file_free, sc_read_record,
                         sc_format_path, sc_select_file, sc_check_sw, SC_ALGORITHM_RSA_RAW,
                         SC_RECORD_BY_REC_NR, SC_PIN_ENCODING_ASCII, SC_READER_SHORT_APDU_MAX_RECV_SIZE, //sc_verify,
                         SC_SEC_ENV_ALG_PRESENT, SC_SEC_ENV_FILE_REF_PRESENT, SC_ALGORITHM_RSA,
                         SC_SEC_ENV_KEY_REF_PRESENT, SC_SEC_ENV_ALG_REF_PRESENT, SC_ALGORITHM_3DES, SC_ALGORITHM_DES,
                         sc_format_apdu, sc_file_new//, sc_get_iso7816_driver
};
#[cfg(not(any(v0_15_0, v0_16_0)))]
use opensc_sys::opensc::{SC_ALGORITHM_AES};
#[cfg(not(any(v0_15_0, v0_16_0, v0_17_0)))]
use opensc_sys::opensc::{SC_SEC_ENV_KEY_REF_SYMMETRIC};
#[cfg(not(any(v0_15_0, v0_16_0, v0_17_0, v0_18_0, v0_19_0)))]
use opensc_sys::opensc::{SC_ALGORITHM_AES_CBC_PAD, SC_ALGORITHM_AES_CBC, SC_ALGORITHM_AES_ECB, sc_sec_env_param,
                         SC_SEC_ENV_PARAM_IV};


use opensc_sys::types::{/*sc_aid, sc_path, SC_MAX_AID_SIZE, SC_MAX_PATH_SIZE, sc_file_t,
    SC_MAX_ATR_SIZE, SC_FILE_TYPE_DF,  */  sc_path, sc_file, sc_apdu, SC_PATH_TYPE_FILE_ID/*, SC_PATH_TYPE_PATH*/,
                        SC_MAX_APDU_BUFFER_SIZE, SC_MAX_PATH_SIZE, //SC_AC_CHV,
                        SC_APDU_FLAGS_CHAINING,
                        SC_APDU_CASE_1, /*SC_APDU_CASE_2_SHORT,*/ SC_APDU_CASE_3_SHORT, SC_APDU_CASE_4_SHORT,
                        SC_PATH_TYPE_DF_NAME, SC_PATH_TYPE_PATH, SC_PATH_TYPE_FROM_CURRENT, SC_PATH_TYPE_PARENT,
                        SC_APDU_CASE_2_SHORT
};
use opensc_sys::log::{sc_do_log, sc_dump_hex, SC_LOG_DEBUG_NORMAL};
use opensc_sys::errors::{sc_strerror, /*SC_ERROR_NO_READERS_FOUND, SC_ERROR_UNKNOWN, SC_ERROR_NO_CARD_SUPPORT, SC_ERROR_NOT_SUPPORTED, */
                         SC_SUCCESS, SC_ERROR_INVALID_ARGUMENTS,
                         SC_ERROR_KEYPAD_MSG_TOO_LONG/*, SC_ERROR_WRONG_PADDING, SC_ERROR_INTERNAL*/
,SC_ERROR_WRONG_LENGTH, SC_ERROR_NOT_ALLOWED, SC_ERROR_FILE_NOT_FOUND, SC_ERROR_INCORRECT_PARAMETERS,
SC_ERROR_OUT_OF_MEMORY, SC_ERROR_UNKNOWN_DATA_RECEIVED
//,SC_ERROR_CARD_CMD_FAILED, SC_ERROR_SECURITY_STATUS_NOT_SATISFIED
};
use opensc_sys::internal::{sc_atr_table};
use opensc_sys::asn1::{sc_asn1_read_tag};
use opensc_sys::iso7816::{ISO7816_TAG_FCI, ISO7816_TAG_FCP};

use crate::wrappers::*;
use crate::constants_types::*;
use crate::se::se_parse_crts;
use crate::path::cut_path;
use crate::missing_exports::me_get_max_recv_size;

use super::{acos5_64_list_files, acos5_64_select_file, acos5_64_set_security_env, acos5_64_process_fci};


pub fn logical_xor(pred1: bool, pred2: bool) -> bool
{
    (pred1 || pred2) && !(pred1 && pred2)
}


/*
In principle, iso7816_select_file is usable in a controlled manner, but if file_out is null, the first shot for an APDU is wrong, the second corrected one is okay,
thus issue a correct APDU right away
The code differs from the C version in 1 line only, where setting apdu.p2 = 0x0C;
*/
fn iso7816_select_file_replacement(card: &mut sc_card, in_path: &sc_path, file_out: *mut *mut sc_file) -> c_int
{
//    let ctx : *mut sc_context;
    let mut apdu : sc_apdu = Default::default();
    let mut buf    = [0u8; SC_MAX_APDU_BUFFER_SIZE];
    let mut pathbuf = [0u8; SC_MAX_PATH_SIZE];
    let mut path = pathbuf.as_mut_ptr();
    let mut r : c_int;
//    let pathlen : c_int;
//    let pathtype : c_int;
    let mut select_mf = 0;
//    let mut file: *mut sc_file = std::ptr::null_mut();
//    let mut buffer : *const c_uchar;
    let mut buffer_len : usize = 0;
    let mut cla : c_uint = 0;
    let mut tag : c_uint = 0;

    let f_log = CStr::from_bytes_with_nul(CRATE).unwrap();
    let fun  = CStr::from_bytes_with_nul(b"iso7816_select_file_replacement\0").unwrap();
/*
    if (card == NULL || in_path == NULL) {
        return SC_ERROR_INVALID_ARGUMENTS;
    }
*/

    let ctx = card.ctx;
    unsafe { copy_nonoverlapping(in_path.value.as_ptr(), path, in_path.len) }; // memcpy(path, in_path->value, in_path->len);
    let mut pathlen : c_int = in_path.len as c_int;
    let mut pathtype = in_path.type_;

    if in_path.aid.len > 0 {
        if pathlen == 0 {
            unsafe { copy_nonoverlapping(in_path.aid.value.as_ptr(), path, in_path.aid.len) }; // memcpy(path, in_path->aid.value, in_path->aid.len);
            pathlen = in_path.aid.len as c_int;
            pathtype = SC_PATH_TYPE_DF_NAME;
        }
        else {
            /* First, select the application */
            unsafe { sc_format_apdu(card, &mut apdu, SC_APDU_CASE_3_SHORT, 0xA4, 4, 0) };
            apdu.data = in_path.aid.value.as_ptr();
            apdu.datalen = in_path.aid.len;
            apdu.lc      = in_path.aid.len;

            r =  unsafe { sc_transmit_apdu(card, &mut apdu) };
//            LOG_TEST_RET(ctx, r, "APDU transmit failed");
            if r < 0 {
                if cfg!(log) {
                    wr_do_log_sds(ctx, f_log, line!(), fun,
                                  CStr::from_bytes_with_nul(b"APDU transmit failed\0").unwrap().as_ptr(),
                                  r,
                                  unsafe { sc_strerror(r) },
                                  CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                }
                return r;
            }
            r = unsafe { sc_check_sw(card, apdu.sw1, apdu.sw2) };
            if r != SC_SUCCESS {
//                LOG_FUNC_RETURN(ctx, r);
                if cfg!(log) {
                    if r < 0 {
                        wr_do_log_sds(ctx, f_log, line!(), fun,
                                      CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                                      r,
                                      unsafe { sc_strerror(r) },
                                      CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                    }
                    else {
                        wr_do_log_t(ctx, f_log, line!(), fun, r,
                                    CStr::from_bytes_with_nul(b"returning with: %d\n\0").unwrap())
                    }
                }
                return r;
            }

            if pathtype == SC_PATH_TYPE_PATH || pathtype == SC_PATH_TYPE_DF_NAME {
                pathtype = SC_PATH_TYPE_FROM_CURRENT;
            }
        }
    }

    unsafe { sc_format_apdu(card, &mut apdu, SC_APDU_CASE_4_SHORT, 0xA4, 0, 0) };

    match pathtype {
        SC_PATH_TYPE_FILE_ID => {
                apdu.p1 = 0;
                if pathlen != 2 {
                    return SC_ERROR_INVALID_ARGUMENTS;
                }
            },
        SC_PATH_TYPE_DF_NAME => {
                apdu.p1 = 4;
            },
        SC_PATH_TYPE_PATH => {
                apdu.p1 = 8;
                if pathlen >= 2 && pathbuf[0]==0x3F && pathbuf[1]==0 {
                    if pathlen == 2 {    /* only 3F00 supplied */
                        select_mf = 1;
                        apdu.p1 = 0;
                    }
                    else {
                        path = unsafe { path.add(2) };
                        pathlen -= 2;
                    }
                }
            },
        SC_PATH_TYPE_FROM_CURRENT => {
                apdu.p1 = 9;
            },
        SC_PATH_TYPE_PARENT => {
                apdu.p1 = 3;
                pathlen = 0;
                apdu.cse = SC_APDU_CASE_2_SHORT;
            },
        _ => {
                r = SC_ERROR_INVALID_ARGUMENTS;
                if cfg!(log) {
                    wr_do_log_sds(ctx, f_log, line!(), fun,
                                  CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                                  r,
                                  unsafe { sc_strerror(r) },
                                  CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                }
                return r;
            },
    }

    apdu.lc = pathlen as usize;
    apdu.data = path;
    apdu.datalen = pathlen as usize;

    if !file_out.is_null() {
        apdu.p2 = 0;        /* first record, return FCI */
        apdu.resp = buf.as_mut_ptr();
        apdu.resplen = buf.len();
        apdu.le = if me_get_max_recv_size(card) < 256 {me_get_max_recv_size(card)} else {256};
    }
    else {
////        apdu.p2 = 0x0C;        /* first record, return nothing */
        apdu.cse = if apdu.lc == 0 {SC_APDU_CASE_1} else {SC_APDU_CASE_3_SHORT};
    }

    r = unsafe { sc_transmit_apdu(card, &mut apdu) };
//    LOG_TEST_RET(ctx, r, "APDU transmit failed");
    if r < 0 {
        if cfg!(log) {
            wr_do_log_sds(ctx, f_log, line!(), fun,
                          CStr::from_bytes_with_nul(b"APDU transmit failed\0").unwrap().as_ptr(),
                          r,
                          unsafe { sc_strerror(r) },
                          CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
        }
        return r;
    }

    if file_out.is_null() {
        /* For some cards 'SELECT' can be only with request to return FCI/FCP. */
        r = unsafe { sc_check_sw(card, apdu.sw1, apdu.sw2) };
        if apdu.sw1 == 0x6A && apdu.sw2 == 0x86 {
            apdu.p2 = 0x00;
            if unsafe { sc_transmit_apdu(card, &mut apdu) } == SC_SUCCESS {
                r = unsafe { sc_check_sw(card, apdu.sw1, apdu.sw2) };
            }
        }
        if apdu.sw1 == 0x61 {
//            LOG_FUNC_RETURN(ctx, SC_SUCCESS);
            r = SC_SUCCESS;
            if cfg!(log) {
                wr_do_log_t(ctx, f_log, line!(), fun, r,
                            CStr::from_bytes_with_nul(b"returning with: %d\n\0").unwrap())
            }
            return r;
        }

//        LOG_FUNC_RETURN(ctx, r);
        if cfg!(log) {
            if r < 0 {
                wr_do_log_sds(ctx, f_log, line!(), fun,
                              CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                              r,
                              unsafe { sc_strerror(r) },
                              CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
            }
            else {
                wr_do_log_t(ctx, f_log, line!(), fun, r,
                            CStr::from_bytes_with_nul(b"returning with: %d\n\0").unwrap())
            }
        }
        return r;
    }

    r = unsafe { sc_check_sw(card, apdu.sw1, apdu.sw2) };
    if r != SC_SUCCESS {
//        LOG_FUNC_RETURN(ctx, r);
        if cfg!(log) {
            if r < 0 {
                wr_do_log_sds(ctx, f_log, line!(), fun,
                              CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                              r,
                              unsafe { sc_strerror(r) },
                              CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
            }
            else {
                wr_do_log_t(ctx, f_log, line!(), fun, r,
                            CStr::from_bytes_with_nul(b"returning with: %d\n\0").unwrap())
            }
        }
        return r;
    }

    if !file_out.is_null() && apdu.resplen == 0 {
        /* For some cards 'SELECT' MF or DF_NAME do not return FCI. */
        if select_mf>0 || pathtype == SC_PATH_TYPE_DF_NAME   {
           let file = unsafe { sc_file_new() };
            if file.is_null() {
//                LOG_FUNC_RETURN(ctx, SC_ERROR_OUT_OF_MEMORY);
                r = SC_ERROR_OUT_OF_MEMORY;
                if cfg!(log) {
                    wr_do_log_sds(ctx, f_log, line!(), fun,
                                  CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                                  r,
                                  unsafe { sc_strerror(r) },
                                  CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                }
                return r;
            }
            unsafe { *file }.path = *in_path;

            unsafe { *file_out = file };
//            LOG_FUNC_RETURN(ctx, SC_SUCCESS);
            r = SC_SUCCESS;
            if cfg!(log) {
                wr_do_log_t(ctx, f_log, line!(), fun, r,
                            CStr::from_bytes_with_nul(b"returning with: %d\n\0").unwrap())
            }
            return r;
        }
    }

    if apdu.resplen < 2 {
//        LOG_FUNC_RETURN(ctx, SC_ERROR_UNKNOWN_DATA_RECEIVED);
        r = SC_ERROR_UNKNOWN_DATA_RECEIVED;
        if cfg!(log) {
            wr_do_log_sds(ctx, f_log, line!(), fun,
                          CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                          r,
                          unsafe { sc_strerror(r) },
                          CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
        }
        return r;
    }

    match unsafe { *apdu.resp } {
        ISO7816_TAG_FCI |
        ISO7816_TAG_FCP => {
            let file = unsafe { sc_file_new() };
            if file.is_null() {
//                LOG_FUNC_RETURN(ctx, SC_ERROR_OUT_OF_MEMORY);
                r = SC_ERROR_OUT_OF_MEMORY;
                if cfg!(log) {
                    wr_do_log_sds(ctx, f_log, line!(), fun,
                                  CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                                  r,
                                  unsafe { sc_strerror(r) },
                                  CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                }
                return r;
            }
            unsafe { (*file).path = *in_path };
/*
            if card->ops->process_fci == NULL {
                sc_file_free(file);
                LOG_FUNC_RETURN(ctx, SC_ERROR_NOT_SUPPORTED);
            }
*/
            let mut buffer : *const c_uchar = apdu.resp;
            r = unsafe { sc_asn1_read_tag(&mut buffer, apdu.resplen, &mut cla, &mut tag, &mut buffer_len) };
            if r == SC_SUCCESS {
                acos5_64_process_fci(card, file, buffer, buffer_len); // card->ops->process_fci(card, file, buffer, buffer_len);
            }
            unsafe { *file_out = file };
        },
        _ => {
//            LOG_FUNC_RETURN(ctx, SC_ERROR_UNKNOWN_DATA_RECEIVED);
                r = SC_ERROR_UNKNOWN_DATA_RECEIVED;
                if cfg!(log) {
                    wr_do_log_sds(ctx, f_log, line!(), fun,
                                  CStr::from_bytes_with_nul(b"returning with\0").unwrap().as_ptr(),
                                  r,
                                  unsafe { sc_strerror(r) },
                                  CStr::from_bytes_with_nul(b"%s: %d (%s)\n\0").unwrap());
                }
                return r;
            }
    }

return SC_SUCCESS;
}

/*
The task of track_iso7816_select_file next to SELECT:
Update card.cache.current_path such that it's always valid (pointing to the currently selected EF/DF),
both before and after the call to iso7816_select_file (even if failing)

same @param and @return as iso7816_select_file
*/
/*
 * What it does
 * @apiNote
 * @param
 * @return
 */
pub fn track_iso7816_select_file(card: &mut sc_card, path: &sc_path, file_out: *mut *mut sc_file) -> c_int
{
    assert_eq!(path.type_, SC_PATH_TYPE_FILE_ID);
    assert_eq!(path.len,   2);
    let file_str = CStr::from_bytes_with_nul(CRATE).unwrap();
    let func     = CStr::from_bytes_with_nul(b"track_iso7816_select_file\0").unwrap();
    let format_1   = CStr::from_bytes_with_nul(b"   called.  curr_type: %d, curr_value: %s\0").unwrap();
    let format_2   = CStr::from_bytes_with_nul(b"              to_type: %d,   to_value: %s\0").unwrap();
    let format_3   = CStr::from_bytes_with_nul(b"returning:  curr_type: %d, curr_value: %s\0").unwrap();
    #[cfg(log)]
    unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(),
                       format_1.as_ptr(), card.cache.current_path.type_,
                       sc_dump_hex(card.cache.current_path.value.as_ptr(), card.cache.current_path.len) ) };
    #[cfg(log)]
    unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(),
                       format_2.as_ptr(), path.type_,
                       sc_dump_hex(path.value.as_ptr(), path.len) ) };

//  let rv = unsafe { (*(*sc_get_iso7816_driver()).ops).select_file.unwrap()(card, path, file_out) };
    let rv = iso7816_select_file_replacement(card, path, file_out);

    /*
    0x6283, SC_ERROR_CARD_CMD_FAILED, "Selected file invalidated" //// Target file has been blocked but selected
    0x6982, SC_ERROR_SECURITY_STATUS_NOT_SATISFIED, "Security status not satisfied" //// Target file has wrong checksum in its header or file is corrupted; probably selected, but inaccessible: test that
    0x6986, SC_ERROR_NOT_ALLOWED,  "Command not allowed (no current EF)" //// No Master File found in card; no MF found
    0x6A82, SC_ERROR_FILE_NOT_FOUND, "File not found" //// Target file not found
    0x6A86, SC_ERROR_INCORRECT_PARAMETERS,"Incorrect parameters P1-P2" //// Invalid P1 or P2. P2 must be 00h and P1 must be 00h or 04h
    0x6A87, SC_ERROR_INCORRECT_PARAMETERS,"Lc inconsistent with P1-P2" //// Wrong P3 length. P3 is not compatible with P1.
      SC_ERROR_CARD_CMD_FAILED if iso7816_check_sw encounters unknown error
    */
    if rv == SC_ERROR_WRONG_LENGTH ||
       rv == SC_ERROR_NOT_ALLOWED  ||
       rv == SC_ERROR_FILE_NOT_FOUND ||
       rv == SC_ERROR_INCORRECT_PARAMETERS /*shouldn't be emitted for select file*/ {
        // select failed, no new card.cache.current_path, do nothing
    }
    else {
/*
        rv == SC_SUCCESS ||
        rv == SC_ERROR_CARD_CMD_FAILED ||
        rv == SC_ERROR_SECURITY_STATUS_NOT_SATISFIED
*/
        if path.value[0..2] != [0x3Fu8, 0xFF][..] {
            let file_id = u16_from_array_begin(&path.value[0..2]);
            let dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
            if file_out.is_null() {
                // TODO
            }
            else {

            }
            assert!(dp.files.contains_key(&file_id));
            let dp_files_value = &dp.files[&file_id];
            card.cache.current_path.value = dp_files_value.0;
            card.cache.current_path.len   = dp_files_value.1[1] as usize;
            card.drv_data = Box::into_raw(dp) as *mut c_void;
        }
    }

    #[cfg(log)]
    unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(),
                       format_3.as_ptr(), card.cache.current_path.type_,
                       sc_dump_hex(card.cache.current_path.value.as_ptr(), card.cache.current_path.len) ) };
    rv
}



/* process path by chunks, 2 byte each and select_file with SC_PATH_TYPE_FILE_ID */
/*
 * What it does
 * @apiNote
 * @param
 * @return
 */
pub fn select_file_by_path(card: &mut sc_card, path: &sc_path, file_out: *mut *mut sc_file) -> c_int
{
    let mut path1 = sc_path { ..*path };
    let rv = cut_path(card, path, &mut path1);
    if rv != SC_SUCCESS {
        return rv;
    }

    let len = path1.len;
    if  len % 2 != 0 {
        return SC_ERROR_INVALID_ARGUMENTS;
    }

    let mut path2 = sc_path { len: 2, ..Default::default() }; // SC_PATH_TYPE_FILE_ID

    for i in 0..len/2 {
        path2.value[0] = path1.value[i*2  ];
        path2.value[1] = path1.value[i*2+1];
        let rv = track_iso7816_select_file(card, &path2, file_out);
        unsafe {
            if (i+1)<len/2 && !file_out.is_null() && !(*file_out).is_null() {
                sc_file_free(*file_out);
                *file_out = std::ptr::null_mut();
            }
        }
        if rv != SC_SUCCESS {
            return rv;
        }
    }
    SC_SUCCESS
}


/*
 * What it does
 * @apiNote
 * @param
 * @return
 */
pub fn enum_dir(card: &mut sc_card, path: &sc_path, only_se_df: bool/*, depth: c_int*/) -> c_int
{
    let f_log = CStr::from_bytes_with_nul(CRATE).unwrap();
    let fun     = CStr::from_bytes_with_nul(b"enum_dir\0").unwrap();
    let fmt   = CStr::from_bytes_with_nul(b"called for path: %s\0").unwrap();
    #[cfg(log)]
    unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, f_log.as_ptr(), line!() as i32, fun.as_ptr(), fmt.as_ptr(),
                       sc_dump_hex(path.value.as_ptr(), path.len) ) };

    let mut dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    assert!(path.len >= 2);
    let file_id = u16_from_array_begin(&path.value[path.len-2..path.len]);
    let fdb = dp.files[&file_id].1[0];
    let dp_files_value = dp.files.entry(file_id).or_insert(([0u8;SC_MAX_PATH_SIZE], [0u8;8], None, None));
    dp_files_value.0    = path.value;
    dp_files_value.1[1] = path.len as u8;
    /* assumes meaningful values in dp_files_value.1 */
    let mrl = dp_files_value.1[4] as usize;
    let nor  = dp_files_value.1[5] as c_uint;
    card.drv_data = Box::into_raw(dp) as *mut c_void;

    if only_se_df  &&  fdb == FDB_SE_FILE && mrl>0 && nor>0
    {
        /* file_out_ptr_mut has the only purpose to invoke scb8 retrieval */
        let mut file_out_ptr_mut = std::ptr::null_mut();
        let mut rv = acos5_64_select_file(card, path, &mut file_out_ptr_mut);
        if !file_out_ptr_mut.is_null() {
            unsafe { sc_file_free(file_out_ptr_mut) };
        }
        assert_eq!(rv, SC_SUCCESS);
        let mut vec_seinfo : Vec<SeInfo> = Vec::new();
        for rec_nr in 1..1+nor {
            let buf = &mut [0u8; 255];
            rv = unsafe { sc_read_record(card, rec_nr, buf.as_mut_ptr(), mrl, SC_RECORD_BY_REC_NR as c_ulong) };
/* * /
// TODO temporary if SE file is pin-protected for READ
if rv < 0 && card.type_== SC_CARD_TYPE_ACOS5_64_V3  // currently has SO_PIN same as User pin
{
    let pin = [0x31u8, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]; //if path.len>4 { [0x31u8, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38] }
//                     else           { [0x38u8, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31] };
    let mut tries_left : c_int = 0;
    rv = unsafe { sc_verify(card, SC_AC_CHV, if path.len>4 {0x81} else {0x01}, pin.as_ptr(), pin.len(), &mut tries_left) };
    assert!(rv == 0);
    rv = unsafe { sc_read_record(card, rec_nr, buf.as_mut_ptr(), mrl, SC_RECORD_BY_REC_NR as c_ulong) };
}
/ * */
            assert!(rv >= 0);
            if rv >= 1 && buf[0] == 0 || rv >= 3 && buf[2] == 0 {
                break;
            }
            if rv >= 3 {
                assert_eq!(rec_nr, buf[2] as u32); // not really required but recommended
            }
            let mut seinfo : SeInfo = Default::default();
            let rv = se_parse_crts(buf[2] as c_int,&buf[3..], &mut seinfo);
            assert!(rv > 0);
            vec_seinfo.push(seinfo);
        }
        assert!(path.len >= 4);
        let file_id_dir = u16_from_array_begin(&path.value[path.len-4..path.len-2]);

        let mut dp : Box<DataPrivate> = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
        let dp_files_value = dp.files.entry(file_id_dir).or_insert(([0u8;SC_MAX_PATH_SIZE], [0u8;8], None, None));
        dp_files_value.3 = Some(vec_seinfo);
        card.drv_data = Box::into_raw(dp) as *mut c_void;
    }
    else if !only_se_df  &&  !is_DFMF(fdb)  &&  fdb != FDB_SE_FILE
    {
        /* file_out_ptr_mut has the only purpose to invoke scb8 retrieval */
        let mut file_out_ptr_mut = std::ptr::null_mut();
        let rv = acos5_64_select_file(card, path, &mut file_out_ptr_mut);
        if !file_out_ptr_mut.is_null() {
            unsafe { sc_file_free(file_out_ptr_mut) };
        }
        assert_eq!(rv, SC_SUCCESS);

        if fdb==FDB_RSA_KEY_EF {// && dp.files[&file_id].2.is_some() && dp.files[&file_id].2.unwrap()[0]==0
            let mut dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };

            if let Some(x) = dp.files.get_mut(&file_id) {
                /* how to distinguish RSAPUB from RSAPRIV without reading ? Assume unconditionally allowed to read: RSAPUB*/
                (*x).1[6] = if (*x).2.unwrap()[0] == 0 {PKCS15_FILE_TYPE_RSAPUBLICKEY} else {PKCS15_FILE_TYPE_RSAPRIVATEKEY};
            }
            card.drv_data = Box::into_raw(dp) as *mut c_void;
        }
    }
    else if is_DFMF(fdb)
    {
        assert!(path.len <= SC_MAX_PATH_SIZE);
        /* file_out_ptr_mut has the only purpose to invoke scb8 retrieval */
        let mut file_out_ptr_mut = std::ptr::null_mut();
        let rv = acos5_64_select_file(card, path, &mut file_out_ptr_mut);
        if !file_out_ptr_mut.is_null() {
            unsafe { sc_file_free(file_out_ptr_mut) };
        }
        assert_eq!(rv, SC_SUCCESS);
        if path.len == 16 {
//            if cfg!(log) {}
            let fmt  = CStr::from_bytes_with_nul(b"### enum_dir: couldn't visit all files due to OpenSC path.len limit.\
 Such deep file system structures are not recommended, nor supported by cos5 with file access control! ###\0").unwrap();
            wr_do_log(card.ctx, f_log, line!(), fun, fmt);
        }
        else {
            let mut files_contained= [0u8; (SC_MAX_APDU_BUFFER_SIZE/2)*2]; // same limit as in opensc-tool (130 file ids)
            let rv = acos5_64_list_files(card, files_contained.as_mut_ptr(), files_contained.len());
            /* * /
                    println!("chunk1 files_contained: {:?}", &files_contained[  ..32]);
                    println!("chunk2 files_contained: {:?}", &files_contained[32..64]);
                    println!("chunk3 files_contained: {:?}", &files_contained[64..96]);
            / * */
            assert!(rv >= 0 && rv%2 == 0);

            for i in 0..(rv/2) as usize {
                let mut tmp_path = *path;
                tmp_path.value[tmp_path.len  ] = files_contained[2*i  ];
                tmp_path.value[tmp_path.len+1] = files_contained[2*i+1];
                tmp_path.len += 2;
//              assert_eq!(tmp_path.len, ((depth+2)*2) as usize);
                enum_dir(card, &tmp_path, only_se_df/*, depth + 1*/);
            }
        }
    }
    SC_SUCCESS
}


/*
 * convert_bytes_tag_fcp_sac_to_scb_array expands the "compressed" tag_fcp_sac (0x8C) bytes from card file/director's
 * header to a 'standard' 8 byte SCB array, interpreting the AM byte (AMB);
 * The position of a SCB within the array is related to a command/operation, that is controlled by this byte
 * The value of SCB refers to a record id in Security Environment file, that stores details of conditions that must be
 * met in order to grant access
 * SC's byte positions are assigned values matching the AM bit-representation in reference manual, i.e. it is reversed
 * to what many other cards do:
 * Bit 7 of AM byte indicates what is stored to byte-index 7 of SC ( Not Used by ACOS )
 * Bit 0 of AM byte indicates what is stored to byte-index 0 of SC ( EF: READ, DF/MF:  Delete_Child )
 * Bits 0,1,2 may have different meaning depending on file type, from bits 3 to 6/7 (unused) meanings are the same for
 * all file types
 * Maybe later integrate this in acos5_64_process_fci
 * @param  bytes_tag_fcp_sac IN  the TLV's V bytes readable from file header for tag 0x8C, same order from left to right;
 *                               number of bytes: min: 0, max. 8
 *                               If there are >= 1 bytes, the first is AM
 * @param  scb8          OUT     8 SecurityConditionBytes, from leftmost (index 0)'READ'/'Delete_Child' to
 *                               (6)'SC_AC_OP_DELETE_SELF', (7)'unused'
 *
 * The reference manual contains a table indicating the possible combinations of bits allowed for a scb:
 * For any violation, None will be returned
 */
/*
 * What it does
 * @apiNote
 * @param
 * @return
 */
pub fn convert_bytes_tag_fcp_sac_to_scb_array(bytes_tag_fcp_sac: &[u8]) -> Result<[u8; 8], c_int>
{
//   assert_eq!(0b0101_1010u16.popcnt(), 4);
    let mut scb8 = [0u8; 8];
    scb8[7] = 0xFF; // though not expected to be accidentally set, it get's overriden to NEVER: it's not used by ACOS

    if bytes_tag_fcp_sac.len() == 0 {
        return Ok(scb8);
    }
    assert!(bytes_tag_fcp_sac.len() <= 8);

    let mut idx = 0;
    let amb = bytes_tag_fcp_sac[idx];
    idx += 1;
    if amb.count_ones() as usize != bytes_tag_fcp_sac.len()-1 { // the count of 1-valued bits of amb Byte must equal (taglen-1), the count of bytes following amb
        return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);
    }

    for pos in 0..8 {
        if (amb & (0b1000_0000 >> pos)) != 0 { //assert(i);we should never get anything for scb8[7], it's not used by ACOS
            scb8[7-pos] = bytes_tag_fcp_sac[idx];
            idx += 1;
        }
    }
    Ok(scb8)
}


/*
 * What it does
 * @apiNote
 * @param
 * @return
 */
pub fn pin_get_policy(card: &mut sc_card, data: &mut sc_pin_cmd_data, tries_left: *mut c_int) -> c_int
{
/* when is AODF read for the pin details info info ? */
    let file_str = CStr::from_bytes_with_nul(CRATE).unwrap();
    let func     = CStr::from_bytes_with_nul(b"pin_get_policy\0").unwrap();
    let format   = CStr::from_bytes_with_nul(CALLED).unwrap();
    #[cfg(log)]
    unsafe {sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(), format.as_ptr())};

    data.pin1.min_length = 4; /* min length of PIN */
    data.pin1.max_length = 8; /* max length of PIN */
    data.pin1.stored_length = 8; /* stored length of PIN */
    data.pin1.encoding = SC_PIN_ENCODING_ASCII; /* ASCII-numeric, BCD, etc */
//  data.pin1.pad_length    = 0; /* filled in by the card driver */
    data.pin1.pad_char = 0xFF;
    data.pin1.offset = 5; /* PIN offset in the APDU */
//  data.pin1.length_offset = 5;
    data.pin1.length_offset = 0; /* Effective PIN length offset in the APDU */

    data.pin1.max_tries = 8;//pin_tries_max; /* Used for signaling back from SC_PIN_CMD_GET_INFO */ /* assume: 8 as factory setting; max allowed number of retries is unretrievable with proper file access condition NEVER read */

    let command = [0x00u8, 0x20, 0x00, data.pin_reference as u8];
    let mut apdu : sc_apdu = Default::default();
    let mut rv = sc_bytes2apdu_wrapper(card.ctx, &command, &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_1);
    rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
    if rv != SC_SUCCESS || apdu.sw1 != 0x63 || (apdu.sw2 & 0xC0) != 0xC0 {
        let format = CStr::from_bytes_with_nul(b"sc_transmit_apdu or 'Get remaining number of retries left for the PIN' \
                     failed\0").unwrap();
        #[cfg(log)]
        unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(),
                            format.as_ptr()) };
        return SC_ERROR_KEYPAD_MSG_TOO_LONG;
    }
    data.pin1.tries_left = (apdu.sw2 & 0x0Fu32) as c_int; //  63 Cnh     n is remaining tries


    if !tries_left.is_null() {
        unsafe { *tries_left = data.pin1.tries_left };
    }
    SC_SUCCESS
}

pub /*const*/ fn acos5_64_atrs_supported() -> [sc_atr_table; 3]
{
    let acos5_64_atrs: [sc_atr_table; 3] = [
        sc_atr_table {
            atr:     CStr::from_bytes_with_nul(ATR_V2).unwrap().as_ptr(),
            atrmask: CStr::from_bytes_with_nul(ATR_MASK).unwrap().as_ptr(),
            name:    CStr::from_bytes_with_nul(NAME_V2).unwrap().as_ptr(),
            type_: SC_CARD_TYPE_ACOS5_64_V2,
            flags: 0,
            card_atr: std::ptr::null_mut(),
        },
        sc_atr_table {
            atr:     CStr::from_bytes_with_nul(ATR_V3).unwrap().as_ptr(),
            atrmask: CStr::from_bytes_with_nul(ATR_MASK).unwrap().as_ptr(),
            name:    CStr::from_bytes_with_nul(NAME_V3).unwrap().as_ptr(),
            type_: SC_CARD_TYPE_ACOS5_64_V3,
            flags: 0,
            card_atr: std::ptr::null_mut(),
        },
        Default::default(),
    ];
    acos5_64_atrs
}

pub fn set_is_running_cmd_long_response(card: &mut sc_card, value: bool)
{
    let mut dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    dp.is_running_cmd_long_response = value;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
}

pub fn get_is_running_cmd_long_response(card: &mut sc_card) -> bool
{
    let dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    let result = dp.is_running_cmd_long_response;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
    result
}

/*
pub fn set_is_running_compute_signature(card: &mut sc_card, value: bool)
{
    let mut dp : Box<DataPrivate> = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    dp.is_running_compute_signature = value;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
}

pub fn get_is_running_compute_signature(card: &mut sc_card) -> bool
{
    let dp : Box<DataPrivate> = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    let result = dp.is_running_compute_signature;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
    result
}
*/
/*
pub fn set_rsa_caps(card: &mut sc_card, value: c_uint)
{
    let mut dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    dp.rsa_caps = value;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
}
*/
/*
pub fn get_rsa_caps(card: &mut sc_card) -> c_uint
{
    let dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    let result = dp.rsa_caps;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
    result
}
*/
pub fn set_sec_env(card: &mut sc_card, value: &sc_security_env)
{
    let mut dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    dp.sec_env = *value;
    // if sc_get_encoding_flags evaluates: secure algorithm flags == 0x0, then set SC_ALGORITHM_RSA_RAW
    dp.sec_env.algorithm_flags = std::cmp::max(dp.sec_env.algorithm_flags, SC_ALGORITHM_RSA_RAW);
    card.drv_data = Box::into_raw(dp) as *mut c_void;
}

pub fn get_sec_env(card: &mut sc_card) -> sc_security_env
{
    let dp : Box<DataPrivate> = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    let result = dp.sec_env;
    card.drv_data = Box::into_raw(dp) as *mut c_void;
    result
}

//TODO integrate this into encrypt_asym
/* this is tailored for a special testing use case, don't use generally, SC_SEC_OPERATION_ENCIPHER_RSAPUBLIC */
pub fn encrypt_public_rsa(card: *mut sc_card, signature: *mut c_uchar, siglen: usize)
{
    let card_ref_mut = unsafe { &mut *card };
    let mut path = Default::default();
    unsafe { sc_format_path(CStr::from_bytes_with_nul(b"3f0041004133\0").unwrap().as_ptr(), &mut path); } // type = SC_PATH_TYPE_PATH;
    let file_ptr_address = std::ptr::null_mut();
    let mut rv = unsafe { sc_select_file(card_ref_mut, &path, file_ptr_address) };
    assert_eq!(rv, SC_SUCCESS);
    let command = [0u8, 0x22, 0x01, 0xB8, 0x0A, 0x80, 0x01, 0x12, 0x81, 0x02, 0x41, 0x33, 0x95, 0x01, 0x80];
    let mut apdu = Default::default();
    rv = sc_bytes2apdu_wrapper(card_ref_mut.ctx, &command, &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_3_SHORT);
    rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
    assert_eq!(rv, SC_SUCCESS);
    let command = [0u8, 0x2A, 0x84, 0x80, 0x02, 0xFF, 0xFF, 0xFF]; // will replace lc, cmd_data, le later; the last 4 bytes are placeholders only for sc_bytes2apdu_wrapper
    apdu = Default::default();
    rv = sc_bytes2apdu_wrapper(card_ref_mut.ctx, &command, &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_4_SHORT);
    let mut rbuf = [0u8; 512];
    assert_eq!(rbuf.len(), siglen);
    apdu.data    = signature;
    apdu.datalen = siglen;
    apdu.lc      = siglen;
    apdu.resp    = rbuf.as_mut_ptr();
    apdu.resplen = siglen;
    apdu.le      = std::cmp::min(siglen, SC_READER_SHORT_APDU_MAX_RECV_SIZE);
    if apdu.lc > card_ref_mut.max_send_size {
        apdu.flags |= SC_APDU_FLAGS_CHAINING as c_ulong;
    }

    set_is_running_cmd_long_response(card_ref_mut, true); // switch to false is done by acos5_64_get_response
    rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
    assert_eq!(rv, SC_SUCCESS);

    println!("signature 'decrypted' with public key:");
    println!("{:X?}", &rbuf[  0.. 32]);
    println!("{:X?}", &rbuf[ 32.. 64]);
    println!("{:X?}", &rbuf[ 64.. 96]);
    println!("{:X?}", &rbuf[ 96..128]);
    println!("{:X?}", &rbuf[128..160]);
    println!("{:X?}", &rbuf[160..192]);
    println!("{:X?}", &rbuf[192..224]);
    println!("{:X?}", &rbuf[224..256]);
    println!("{:X?}", &rbuf[256..288]);
    println!("{:X?}", &rbuf[288..320]);
    println!("{:X?}", &rbuf[320..352]);
    println!("{:X?}", &rbuf[352..384]);
    println!("{:X?}", &rbuf[384..416]);
    println!("{:X?}", &rbuf[416..448]);
    println!("{:X?}", &rbuf[448..480]);
    println!("{:X?}", &rbuf[480..512]);
}

pub fn encrypt_asym(card: &mut sc_card, crypt_data: &mut CardCtl_generate_crypt_asym, print: bool) -> c_int
{
    /*  don't use print==true: it's a special, tailored case (with some hard-code crypt_data) for testing purposes */
    let mut rv;
    let mut env = sc_security_env {
        operation: SC_SEC_OPERATION_ENCIPHER_RSAPUBLIC,
        flags    : SC_SEC_ENV_FILE_REF_PRESENT.into(),
        algorithm: SC_ALGORITHM_RSA,
        file_ref: sc_path { len: 2, ..Default::default() }, // file_ref.value[0..2] = fidRSApublic.getub2;
        ..Default::default()
    };
    if crypt_data.perform_mse {
        env.file_ref.value[0] = (crypt_data.file_id_pub >> 8  ) as c_uchar;
        env.file_ref.value[1] = (crypt_data.file_id_pub & 0xFF) as c_uchar;
//        command = [0u8, 0x22, 0x01, 0xB8, 0x0A, 0x80, 0x01, 0x12, 0x81, 0x02, (crypt_data.file_id_pub >> 8) as c_uchar, (crypt_data.file_id_pub & 0xFF) as c_uchar, 0x95, 0x01, 0x80];
    }
    else if print {
        env.file_ref.value[0] = 0x41;
        env.file_ref.value[1] = 0x33;
        let mut path = Default::default();
        let mut file_ptr = std::ptr::null_mut();
        unsafe { sc_format_path(CStr::from_bytes_with_nul(b"3f0041004133\0").unwrap().as_ptr(), &mut path); } // path.type_ = SC_PATH_TYPE_PATH;
        rv = unsafe { sc_select_file(card, &path, &mut file_ptr) };
        assert_eq!(rv, SC_SUCCESS);
//        command = [0u8, 0x22, 0x01, 0xB8, 0x0A, 0x80, 0x01, 0x12, 0x81, 0x02, 0x41, 0x33, 0x95, 0x01, 0x80];
    }

    if crypt_data.perform_mse || print {
        rv = acos5_64_set_security_env(card, &env, 0);
        if rv < 0 {
            /*
                            mixin (log!(__FUNCTION__,  "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC"));
                            hstat.SetString(IUP_TITLE, "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC");
                            return IUP_DEFAULT;
            */
            return rv;
        }
    }
    let command = [0u8, 0x2A, 0x84, 0x80, 0x02, 0xFF, 0xFF, 0xFF]; // will replace lc, cmd_data, le later; the last 4 bytes are placeholders only for sc_bytes2apdu_wrapper
    let mut apdu = Default::default();
    rv = sc_bytes2apdu_wrapper(card.ctx, &command, &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_4_SHORT);
    let mut rbuf = [0u8; 512];
 //   assert_eq!(rbuf.len(), siglen);
    apdu.data    = crypt_data.data.as_ptr();
    apdu.datalen = crypt_data.data_len;
    apdu.lc      = crypt_data.data_len;
    apdu.resp    = rbuf.as_mut_ptr();
    apdu.resplen = rbuf.len();
    apdu.le      = std::cmp::min(crypt_data.data_len, SC_READER_SHORT_APDU_MAX_RECV_SIZE);
    if apdu.lc > card.max_send_size {
        apdu.flags |= SC_APDU_FLAGS_CHAINING as c_ulong;
    }

    set_is_running_cmd_long_response(card, true); // switch to false is done by acos5_64_get_response
    rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.resplen, crypt_data.data_len);
    let dst = &mut crypt_data.data[.. crypt_data.data_len];
    dst.copy_from_slice(&rbuf[.. crypt_data.data_len]);

    if print {
        println!("signature 'decrypted' with public key:");
        println!("{:X?}", &rbuf[0..32]);
        println!("{:X?}", &rbuf[32..64]);
        println!("{:X?}", &rbuf[64..96]);
        println!("{:X?}", &rbuf[96..128]);
        println!("{:X?}", &rbuf[128..160]);
        println!("{:X?}", &rbuf[160..192]);
        println!("{:X?}", &rbuf[192..224]);
        println!("{:X?}", &rbuf[224..256]);
        println!("{:X?}", &rbuf[256..288]);
        println!("{:X?}", &rbuf[288..320]);
        println!("{:X?}", &rbuf[320..352]);
        println!("{:X?}", &rbuf[352..384]);
        println!("{:X?}", &rbuf[384..416]);
        println!("{:X?}", &rbuf[416..448]);
        println!("{:X?}", &rbuf[448..480]);
        println!("{:X?}", &rbuf[480..512]);
    }
    0
}

pub fn generate_asym(card: &mut sc_card, data: &mut CardCtl_generate_crypt_asym) -> c_int
{
    let f_log = CStr::from_bytes_with_nul(CRATE).unwrap();
    let fun  = CStr::from_bytes_with_nul(b"generate_asym\0").unwrap();
    if cfg!(log) {
        let fmt  = CStr::from_bytes_with_nul(CALLED).unwrap();
        wr_do_log(card.ctx, f_log, line!(), fun, fmt);
    }
    let mut rv;

    if data.perform_mse {
        let mut env = sc_security_env {
            operation: SC_SEC_OPERATION_GENERATE_RSAPRIVATE,
            flags    : (SC_SEC_ENV_ALG_PRESENT | SC_SEC_ENV_FILE_REF_PRESENT).into(),
            algorithm: SC_ALGORITHM_RSA,
            file_ref: sc_path { len: 2, ..Default::default() }, // file_ref.value[0..2] = fidRSAprivate.getub2;
            ..Default::default()
        };
        env.file_ref.value[0] = (data.file_id_priv >> 8  ) as c_uchar;
        env.file_ref.value[1] = (data.file_id_priv & 0xFF) as c_uchar;
        rv = acos5_64_set_security_env(card, &env, 0);
        if rv < 0 {
/*
                mixin (log!(__FUNCTION__,  "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPRIVATE"));
                hstat.SetString(IUP_TITLE, "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPRIVATE");
                return IUP_DEFAULT;
*/
            return rv;
        }

        let mut env = sc_security_env {
            operation: SC_SEC_OPERATION_GENERATE_RSAPUBLIC,
            flags    : (SC_SEC_ENV_ALG_PRESENT | SC_SEC_ENV_FILE_REF_PRESENT).into(),
            algorithm: SC_ALGORITHM_RSA,
            file_ref: sc_path { len: 2, ..Default::default() }, // file_ref.value[0..2] = fidRSApublic.getub2;
            ..Default::default()
        };
        env.file_ref.value[0] = (data.file_id_pub >> 8  ) as c_uchar;
        env.file_ref.value[1] = (data.file_id_pub & 0xFF) as c_uchar;
        rv = acos5_64_set_security_env(card, &env, 0);
        if rv < 0 {
/*
                mixin (log!(__FUNCTION__,  "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC"));
                hstat.SetString(IUP_TITLE, "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC");
                return IUP_DEFAULT;
*/
            return rv;
        }
    }
    let mut command = [0u8, 0x46, 0,0,18, data.key_len_code, data.key_priv_type_code, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
    if data.exponent_std { command[4] = 2; }
    let mut apdu = Default::default();
    rv = sc_bytes2apdu_wrapper(card.ctx, &command[.. if data.exponent_std  {command.len()-16} else {command.len()}], &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_3_SHORT);
    let fmt  = CStr::from_bytes_with_nul(b"generate_asym: %s\0").unwrap();
    unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, f_log.as_ptr(), line!() as c_int, fun.as_ptr(), fmt.as_ptr(),
                       sc_dump_hex(command.as_ptr(), 7)) };
    rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
//    data.op_success = rv==SC_SUCCESS;
    rv
}


/*
  The EMSA-PKCS1-v1_5 DigestInfo prefix (all content excluding the trailing hash) is known, same the length of hash
  guess by length of known length of DigestInfo, whether the input likely is a DigestInfo and NOT some other raw data
*/
pub fn is_any_of_di_by_len(len: usize) -> bool
{
   let known_len = [34u8, 35, 47, 51, 67, 83];
    for i in 0..known_len.len() {
        if known_len[i] as usize == len { return true; }
    }
    false
}

pub fn trailing_blockcipher_padding_calculate(
    block_size   : c_uchar, // 16 or 8
    padding_type : c_uchar, // any of BLOCKCIPHER_PAD_TYPE_*
    rem          : c_uchar  // == len (input len to blockcipher encrypt, may be != block_size) % block_size; 0 <= rem < block_size
) -> Vec<c_uchar> // in general: 0 <= result_len <= block_size, but different for some padding_type
{
    assert!(rem < block_size);
    assert!(block_size == 16 || block_size == 8);
    assert!([BLOCKCIPHER_PAD_TYPE_ZEROES, BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5,
        BLOCKCIPHER_PAD_TYPE_PKCS5, BLOCKCIPHER_PAD_TYPE_ANSIX9_23/*, BLOCKCIPHER_PAD_TYPE_W3C*/].contains(&padding_type));
    let mut vec : Vec<c_uchar> = Vec::with_capacity(block_size.into());
    match padding_type {
        BLOCKCIPHER_PAD_TYPE_ZEROES => {
            for _i in 0..block_size- if rem==0 {block_size} else {rem}
                { vec.push(0x00); }
            },
        BLOCKCIPHER_PAD_TYPE_ONEANDZEROES => {
            vec.push(0x80);
            for _i in 0..block_size-rem-1 { vec.push(0x00); }
        },
        BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5 => {
            if rem != 0 {
                vec.push(0x80);
                for _i in 0..block_size-rem-1 { vec.push(0x00); }
            }
        },
        BLOCKCIPHER_PAD_TYPE_PKCS5 => {
            let pad_byte = block_size-rem;
            for _i in 0..pad_byte { vec.push(pad_byte); }
        },
        BLOCKCIPHER_PAD_TYPE_ANSIX9_23 => {
            let pad_byte = block_size-rem;
            for _i in 0..pad_byte-1 { vec.push(0x00); }
            vec.push(pad_byte);

        },
/*
        BLOCKCIPHER_PAD_TYPE_W3C => {

        },
*/
        _ => ()
    }
    vec
}

pub fn trailing_blockcipher_padding_get_length(
    block_size   : c_uchar, // 16 or 8
    padding_type : c_uchar, // any of BLOCKCIPHER_PAD_TYPE_*
    last_block_values: &[c_uchar]
) -> Result<c_uchar,c_int> // in general: 0 <= result_len <= block_size, but different for some padding_type
{
    assert_eq!(usize::from(block_size), last_block_values.len());
    match padding_type {
        BLOCKCIPHER_PAD_TYPE_ZEROES => {
            let mut cnt = 0u8;
            for b in last_block_values.iter().rev() {
                if *b==0 { cnt += 1; }
                else {
                    break;
                }
            }
            if cnt==block_size {return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);}
            Ok(cnt)
        },
        BLOCKCIPHER_PAD_TYPE_ONEANDZEROES => {
            let mut cnt = 0u8;
            for b in last_block_values.iter().rev() {
                if *b==0 { cnt += 1; }
                else {
                    if *b!=0x80 {return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);}
                    cnt += 1;
                    break;
                }
            }
            if cnt==block_size && last_block_values[0]==0 {return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);}
            Ok(cnt)
        },
        BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5 => {
            /* last byte 0x80 will be interpreted as padding, thus plaintext data can't end with 0x80 ! TODO possibly check while encrypting for trailing byte 0x80 */
            if ![0u8, 0x80].contains(&last_block_values[usize::from(block_size-1)]) {return Ok(0);}
            let mut cnt = 0u8;
            for b in last_block_values.iter().rev() {
                if *b==0 { cnt += 1; }
                else {
                    if *b!=0x80 {/*what to do now? assume wrong padding or payload?*/ return Ok(0)/*Err(SC_ERROR_KEYPAD_MSG_TOO_LONG)*/;}
                    cnt += 1;
                    break;
                }
            }
            if cnt==block_size && [0u8, 0x80].contains(&last_block_values[0]) {return Ok(0)/*Err(SC_ERROR_KEYPAD_MSG_TOO_LONG)*/;}
            Ok(cnt)
        },
        BLOCKCIPHER_PAD_TYPE_PKCS5 => {
            let pad_byte = last_block_values[last_block_values.len()-1];
            let mut cnt = 1u8;
            for (i,b) in last_block_values[..usize::from(block_size-1)].iter().rev().enumerate() {
                if *b==pad_byte && i+1<usize::from(pad_byte) { cnt += 1; }
                else {break;}
            }
            if cnt != pad_byte {return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);}
            Ok(cnt)
        },
        BLOCKCIPHER_PAD_TYPE_ANSIX9_23 => {
            let pad_byte = last_block_values[last_block_values.len()-1];
            let mut cnt = 1u8;
            for (i,b) in last_block_values[..usize::from(block_size-1)].iter().rev().enumerate() {
                if *b==0 && i+1<usize::from(pad_byte) { cnt += 1; }
                else {break;}
            }
            if cnt != pad_byte {return Err(SC_ERROR_KEYPAD_MSG_TOO_LONG);}
            Ok(cnt)
        },
/*
        BLOCKCIPHER_PAD_TYPE_W3C => {
Ok(0)
        },
*/
        _ => Err(SC_ERROR_KEYPAD_MSG_TOO_LONG)
    }
}


#[allow(non_snake_case)]
fn multipleGreaterEqual(multiplier: usize, x: usize) -> usize
{
    let rem = x % multiplier;
    x + if rem==0 {0} else {multiplier-rem}
}

#[allow(non_snake_case)]
#[cfg(not(any(v0_15_0, v0_16_0)))]
fn algo_ref_cos5_sym_SEDO(algo: c_uint, op_mode_cbc: bool) -> c_uint
{
    match algo {
        SC_ALGORITHM_AES => if op_mode_cbc {6} else {4},
        SC_ALGORITHM_3DES=> if op_mode_cbc {2} else {0},
        SC_ALGORITHM_DES => if op_mode_cbc {3} else {1},
        _ => 0xFFFF_FFFF,
    }
}

fn vecu8_from_file(path_ptr: *const c_char) -> std::io::Result<Vec<u8>>
{
    if path_ptr.is_null() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "oh no!"));
    }
    let path_str = match unsafe { CStr::from_ptr(path_ptr).to_str() } {
        Ok(path) => path,
        Err(_e) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "oh no!")),
    };
    fs::read(path_str)
}

/*
7.4.3.6.  Symmetric Key Encrypt does    work with chaining for CryptoMate64;                               CryptoMate Nano say's, it doesn't support chaining
7.4.3.7.  Symmetric Key Decrypt doesn't work with chaining for CryptoMate64, though it should per ref.man; CryptoMate Nano say's, it doesn't support chaining
if inData is not a multiple of blockSize, then addPadding80 will be done and outData must be able to receive that
*/
/* This function cares for padding the input TODO */
/* Acc to ref. manual, V2.00 uses chaining, while V3.00 does not !
https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Cipher_Block_Chaining_(CBC)

*/
#[allow(non_snake_case)]
pub fn sym_en_decrypt(card: &mut sc_card, crypt_sym: &mut CardCtl_crypt_sym) -> c_int
{
    let f_log = CStr::from_bytes_with_nul(CRATE).unwrap();
    let fun  = CStr::from_bytes_with_nul(b"sym_en_decrypt\0").unwrap();
    if cfg!(log) {
        let fmt_enc  = CStr::from_bytes_with_nul(b"called for encryption\0").unwrap();
        let fmt_dec  = CStr::from_bytes_with_nul(b"called for decryption\0").unwrap();
        wr_do_log(card.ctx, f_log, line!(), fun,if crypt_sym.encrypt {fmt_enc} else {fmt_dec});
    }

    let indata_len;
    let indata_ptr;
    let mut vec_in = Vec::new();

    if !crypt_sym.infile.is_null() {
        vec_in.extend_from_slice(match vecu8_from_file(crypt_sym.infile) {
            Ok(vec) => vec,
            Err(e) => return e.raw_os_error().unwrap(),
        }.as_ref());
        indata_len = vec_in.len();
        indata_ptr = vec_in.as_ptr();
    }
    else {
        indata_len = std::cmp::min(crypt_sym.indata_len, crypt_sym.indata.len());
        indata_ptr = crypt_sym.indata.as_ptr();
    }

    let mut rv;
    let block_size = usize::from(crypt_sym.block_size);
    let Len1 = indata_len;
    let Len0 = /*multipleLessEqual*/ (Len1/block_size) * block_size;
    let Len2 = multipleGreaterEqual(block_size, Len1+
        if !crypt_sym.encrypt || [BLOCKCIPHER_PAD_TYPE_ZEROES, BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5].contains(&crypt_sym.pad_type) {0} else {1});
    if !crypt_sym.encrypt {
        assert_eq!(Len1, Len0);
        assert_eq!(Len1, Len2);
    }

    let outdata_len;
    let outdata_ptr;
    let mut vec_out = Vec::new();

    if !crypt_sym.outfile.is_null() {
        vec_out.resize(Len2, 0u8);
        outdata_len = Len2;
        outdata_ptr = vec_out.as_mut_ptr();
    }
    else {
        outdata_len = std::cmp::min(crypt_sym.outdata_len, crypt_sym.outdata.len());
        outdata_ptr = crypt_sym.outdata.as_mut_ptr();
    }
/* */
    if cfg!(log) {
        assert!(indata_len >= 32);
        let fmt  = CStr::from_bytes_with_nul(b"called with indata_len: %zu, first 16 bytes: %s\0").unwrap();
        wr_do_log_tu(card.ctx, f_log, line!(), fun, indata_len, unsafe {sc_dump_hex(indata_ptr, 32)}, fmt);
        let fmt  = CStr::from_bytes_with_nul(b"called with infile_name: %s, outfile_name: %s\0").unwrap();
        wr_do_log_tt(card.ctx, f_log, line!(), fun, crypt_sym.infile, crypt_sym.outfile, fmt);
    }
/* */

    if !crypt_sym.infile.is_null() && !crypt_sym.outfile.is_null()
    { assert_ne!(crypt_sym.infile, crypt_sym.outfile); } // FIXME doesn't work for symbolic links: the check is meant for using copy_nonoverlapping
    assert!(Len1 == 0    || outdata_len >= Len1);
    assert!(Len1 == Len2 || outdata_len == Len2);
    let mut inDataRem = Vec::with_capacity(block_size);
    if crypt_sym.encrypt && Len1 != Len2 {
        inDataRem.resize(Len1-Len0, 0u8);
        unsafe { copy_nonoverlapping(indata_ptr.add(Len0), inDataRem.as_mut_ptr(), Len1-Len0) };
        inDataRem.extend_from_slice(trailing_blockcipher_padding_calculate(crypt_sym.block_size, crypt_sym.pad_type, (Len1-Len0) as u8).as_slice() );
        assert_eq!(inDataRem.len(), block_size);
    }

    let mut env : sc_security_env =  Default::default();
    #[cfg(not(any(v0_15_0, v0_16_0)))] // due to SC_ALGORITHM_AES
    {
    if crypt_sym.perform_mse {
        /* Security Environment */
        #[cfg(not(any(v0_15_0, v0_16_0, v0_17_0, v0_18_0, v0_19_0)))]
        let sec_env_param;
        env = sc_security_env {
            operation: if crypt_sym.encrypt {SC_SEC_OPERATION_ENCIPHER_SYMMETRIC} else {SC_SEC_OPERATION_DECIPHER_SYMMETRIC},
            flags    : (SC_SEC_ENV_KEY_REF_PRESENT | SC_SEC_ENV_ALG_REF_PRESENT | SC_SEC_ENV_ALG_PRESENT).into(),
            algorithm: if crypt_sym.block_size==16 {SC_ALGORITHM_AES} else {SC_ALGORITHM_3DES /*TODO should I ever consider supporting SC_ALGORITHM_DES*/},
            key_ref: [crypt_sym.key_ref, 0,0,0,0,0,0,0],
            key_ref_len: 1,
            algorithm_ref: algo_ref_cos5_sym_SEDO(if crypt_sym.block_size==16 {SC_ALGORITHM_AES} else {SC_ALGORITHM_3DES}, crypt_sym.cbc),

            ..Default::default()
        };
        #[cfg(not(any(v0_15_0, v0_16_0, v0_17_0)))]
            { env.flags |= SC_SEC_ENV_KEY_REF_SYMMETRIC as c_ulong; }
        #[cfg(not(any(v0_15_0, v0_16_0, v0_17_0, v0_18_0, v0_19_0)))]
            {
                if (env.algorithm & SC_ALGORITHM_AES) > 0 {
                    if !crypt_sym.cbc { env.algorithm_flags |= SC_ALGORITHM_AES_ECB; }
                    else {
                        if crypt_sym.pad_type == BLOCKCIPHER_PAD_TYPE_PKCS5
                        { env.algorithm_flags |= SC_ALGORITHM_AES_CBC_PAD; }
                        else
                        { env.algorithm_flags |= SC_ALGORITHM_AES_CBC; }
                    }
                }

                if crypt_sym.iv_len != 0 {
                    assert_eq!(crypt_sym.iv_len, block_size);
                    sec_env_param = sc_sec_env_param {
                        param_type: SC_SEC_ENV_PARAM_IV,
                        value: crypt_sym.iv.as_mut_ptr() as *mut c_void,
                        value_len: crypt_sym.iv_len as c_uint
                    };
                    env.params[0] = sec_env_param;
                }
            }
        rv = acos5_64_set_security_env(card, &env, 0);
        if rv < 0 {
            /*
              mixin (log!(__FUNCTION__,  "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC"));
              hstat.SetString(IUP_TITLE, "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC");
              return IUP_DEFAULT;
            */
            return rv;
        }
    }}

    /* encrypt / decrypt */
    let mut first = true;
    let max_send = 256usize - block_size;
    let command : [u8; 7] = [if !crypt_sym.cbc || (Len1==Len2 && Len1<=max_send) {0u8} else {0x10u8}, 0x2A,
        if crypt_sym.encrypt {0x84u8} else {0x80u8}, if crypt_sym.encrypt {0x80u8} else {0x84u8}, 0x01, 0xFF, 0xFF];
    let mut apdu = Default::default();
    rv = sc_bytes2apdu_wrapper(card.ctx, &command, &mut apdu);
    assert_eq!(rv, SC_SUCCESS);
    assert_eq!(apdu.cse, SC_APDU_CASE_4_SHORT);
    let mut cnt = 0usize; // counting apdu.resplen bytes received;
    let mut path = Default::default();
    /* select currently selected DF (clear accumulated CRT) */
    unsafe { sc_format_path(CStr::from_bytes_with_nul(b"3FFF\0").unwrap().as_ptr(), &mut path); }

    while cnt < Len0 || (cnt == Len0 && Len1 != Len2) {
        if first { first = false; }
        else if crypt_sym.cbc && !crypt_sym.encrypt {
            rv = unsafe { sc_select_file(card, &path, std::ptr::null_mut()) }; // clear accumulated CRT
            assert_eq!(rv, SC_SUCCESS);
            rv = acos5_64_set_security_env(card, &env, 0);
            if rv < 0 {
                /*

                tlv_new[posIV..posIV+blockSize] = inData[cnt-blockSize..cnt];

                  mixin (log!(__FUNCTION__,  "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC"));
                  hstat.SetString(IUP_TITLE, "acos5_64_set_security_env failed for SC_SEC_OPERATION_GENERATE_RSAPUBLIC");
                  return IUP_DEFAULT;
                */
                return rv;
            }
        }
        if cnt < Len0 {
            if crypt_sym.cbc && Len1==Len2 && Len0-cnt<=max_send { apdu.cla  = 0; }
            apdu.data = unsafe { indata_ptr.add(cnt) };
            apdu.datalen = std::cmp::min(max_send, Len0-cnt);
            #[cfg(not(any(v0_15_0, v0_16_0, v0_17_0, v0_18_0, v0_19_0)))]
            {
                /* correct IV for next loop cycle */
                if crypt_sym.cbc && !crypt_sym.encrypt {
                    env.params[0].value = unsafe { indata_ptr.add(cnt + apdu.datalen - block_size) as *mut c_void };
                }
            }
        }
        else {
            apdu.cla  = 0;
            apdu.data    = inDataRem.as_ptr();
            apdu.datalen = inDataRem.len();
        }
        apdu.lc = apdu.datalen;
        apdu.le = apdu.datalen;
        apdu.resp = unsafe { outdata_ptr.add(cnt) };
        apdu.resplen = outdata_len-cnt;
        rv = unsafe { sc_transmit_apdu(card, &mut apdu) };
        if rv != SC_SUCCESS  { return rv; }
        rv = unsafe { sc_check_sw(card, apdu.sw1, apdu.sw2) };
        if rv != SC_SUCCESS  { return rv; }
        if apdu.resplen == 0 { return SC_ERROR_KEYPAD_MSG_TOO_LONG; }
        assert_eq!(apdu.datalen, apdu.resplen);
        cnt += apdu.datalen;
    }

    if crypt_sym.encrypt {
        crypt_sym.outdata_len = cnt;
    }
    else {
        let mut last_block_values = [0u8; 16];
        unsafe { copy_nonoverlapping(outdata_ptr.add(cnt-block_size), last_block_values.as_mut_ptr(), block_size) };

        crypt_sym.outdata_len = cnt-trailing_blockcipher_padding_get_length(crypt_sym.block_size, crypt_sym.pad_type,
            &last_block_values[..block_size]).unwrap() as usize;
        if !crypt_sym.outfile.is_null() {
            vec_out.truncate(crypt_sym.outdata_len);
        }
    }

    if !crypt_sym.outfile.is_null() {
        let path = unsafe { CStr::from_ptr(crypt_sym.outfile) };
        let path_str = match path.to_str() {
            Ok(path_str) => path_str,
            Err(e) => return e.valid_up_to() as c_int,
        };
        match fs::write(path_str, vec_out) {
            Ok(_) => (),
            Err(e) => return e.raw_os_error().unwrap(),
        }
    }

    crypt_sym.outdata_len as c_int
}


pub fn get_files_hashmap_info(card: &mut sc_card, key: u16) -> Result<[u8; 32], c_int>
{
    let file_str = CStr::from_bytes_with_nul(CRATE).unwrap();
    let func     = CStr::from_bytes_with_nul(b"get_files_hashmap_info\0").unwrap();
    let format   = CStr::from_bytes_with_nul(CALLED).unwrap();
    #[cfg(log)]
        unsafe {sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(), format.as_ptr())};

    let mut rbuf = [0u8; 32];
    let dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
/*
alias  TreeTypeFS = tree_k_ary.Tree!ub32; // 8 bytes + length of pathlen_max considered (, here SC_MAX_PATH_SIZE = 16) + 8 bytes SAC (file access conditions)
                            path                    File Info       scb8                SeInfo
pub type ValueTypeFiles = ([u8; SC_MAX_PATH_SIZE], [u8; 8], Option<[u8; 8]>, Option<Vec<SeInfo>>);
File Info originally:  {FDB, DCB, FILE ID, FILE ID, SIZE or MRL, SIZE or NOR, SFI, LCSI}
File Info actually:    {FDB, *,   FILE ID, FILE ID, *,           *,           *,   LCSI}
*/
    if dp.files.contains_key(&key) {
        let dp_files_value_ref = &dp.files[&key];
        {
            let dst = &mut rbuf[ 0.. 8];
            dst.copy_from_slice(&dp_files_value_ref.1);
        }
        {
            let dst = &mut rbuf[ 8..24];
            dst.copy_from_slice(&dp_files_value_ref.0);
        }
        if dp_files_value_ref.2.is_some() {
            let dst = &mut rbuf[24..32];
            dst.copy_from_slice(&dp_files_value_ref.2.unwrap());
        }
        else {
            let format = CStr::from_bytes_with_nul(b"### forgot to call update_hashmap first ###\0").unwrap();
            #[cfg(log)]
                unsafe { sc_do_log(card.ctx, SC_LOG_DEBUG_NORMAL, file_str.as_ptr(), line!() as i32, func.as_ptr(),
                                   format.as_ptr()) };
        }
    }
    else {
        return Err(SC_ERROR_FILE_NOT_FOUND);
    }

    card.drv_data = Box::into_raw(dp) as *mut c_void;
    Ok(rbuf)
}


// when update_hashmap returns all entries have: 1. path, 2. File Info: [u8; 8], 3. scb8: Option<[u8; 8]>.is_some, 4. for DF s, SeInfo: Option<Vec<SeInfo>>.is_some
/// The function ensures, that
///   all dp.files[?].2 are Some, and
///   all dp.files[?].1[6] are set for internal EF +? (this currently doesn't include detecting file content matches the OpenSC-implemented PKCS#15
///   conformance; OpenSC is not 2016:ISO/IEC 7816-15 compliant)
///
/// Possibly this function will be followed by another one that does the PKCS#15 introspection into files to detect the type, thus moving the
/// over-complicated code from acos5_64_gui to the driver and overhaul that
/// @apiNote  Called from acos5_64_gui and ? (pccs15_init sanity_check ?)
/// @param    card
pub fn update_hashmap(card: &mut sc_card) {
    let f_log = CStr::from_bytes_with_nul(CRATE).unwrap();
    let fun  = CStr::from_bytes_with_nul(b"update_hashmap\0").unwrap();
    if cfg!(log) {
        wr_do_log(card.ctx, f_log, line!(), fun, CStr::from_bytes_with_nul(CALLED).unwrap());
    }
    let mut path = Default::default();
    unsafe { sc_format_path(CStr::from_bytes_with_nul(b"3F00\0").unwrap().as_ptr(), &mut path); } // type = SC_PATH_TYPE_PATH;
    let rv = enum_dir(card, &path, false/*, 0*/);
    assert_eq!(rv, SC_SUCCESS);
/* * /
    let dp = unsafe { Box::from_raw(card.drv_data as *mut DataPrivate) };
    let fmt1  = CStr::from_bytes_with_nul(b"key: %04X, val.1: %s\0").unwrap();
    let fmt2  = CStr::from_bytes_with_nul(b"key: %04X, val.2: %s\0").unwrap();
    for (key, val) in dp.files.iter() {
        if val.2.is_some() {
            wr_do_log_tu(card.ctx, f_log, line!(), fun, *key, unsafe { sc_dump_hex(val.1.as_ptr(), 8) }, fmt1);
            wr_do_log_tu(card.ctx, f_log, line!(), fun, *key, unsafe { sc_dump_hex(val.2.unwrap().as_ptr(), 8) }, fmt2);
        }
    }
    card.drv_data = Box::into_raw(dp) as *mut c_void;
/ * */
    if cfg!(log) {
        wr_do_log(card.ctx, f_log, line!(), fun, CStr::from_bytes_with_nul(RETURNING).unwrap());
    }
}



#[cfg(test)]
mod tests {
    use super::{convert_bytes_tag_fcp_sac_to_scb_array, multipleGreaterEqual, trailing_blockcipher_padding_calculate,
                trailing_blockcipher_padding_get_length};
    use crate::constants_types::*;
//    use opensc_sys::errors::*;

    #[test]
    fn test_convert_bytes_tag_fcp_sac_to_scb_array() {
        // the complete TLV : [0x8C, 0x07,  0x7D, 0x02, 0x03, 0x04, 0xFF, 0xFF, 0x02]
        let bytes_tag_fcp_sac = [0x7D, 0x02, 0x03, 0x04, 0xFF, 0xFF, 0x02];
        let mut scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x02, 0x00, 0xFF, 0xFF, 0x04, 0x03, 0x02, 0xFF]);

        let bytes_tag_fcp_sac : [u8; 0] = [];
        scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF]);

        let bytes_tag_fcp_sac = [0x00];
        scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF]);

        let bytes_tag_fcp_sac = [0x7F, 0xFF, 0xFF, 0x03, 0x03, 0x01, 0x03, 0x01];
        scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x01, 0x03, 0x01, 0x03, 0x03, 0xFF, 0xFF, 0xFF]);

        let bytes_tag_fcp_sac = [0x62, 0x06, 0x05, 0x01];
        scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x06, 0xFF]);

        let bytes_tag_fcp_sac = [0x2B, 0x05, 0x03, 0x01, 0x45];
        scb8 = convert_bytes_tag_fcp_sac_to_scb_array(&bytes_tag_fcp_sac).unwrap();
        assert_eq!(scb8, [0x45, 0x01, 0x00, 0x03, 0x00, 0x05, 0x00, 0xFF]);
    }

    #[allow(non_snake_case)]
    #[test]
    fn test_multipleGreaterEqual() {
        assert_eq!(multipleGreaterEqual(8, 0),  0);
        assert_eq!(multipleGreaterEqual(8, 7),  8);
        assert_eq!(multipleGreaterEqual(8, 8),  8);
        assert_eq!(multipleGreaterEqual(8, 9), 16);
    }

    #[test]
    fn test_trailing_blockcipher_padding_calculate() {
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ZEROES, 3).as_slice(), &[0u8,0,0,0,0]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ZEROES, 7).as_slice(), &[0u8]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ZEROES, 0).as_slice(), &[0u8; 0]);

        // this is implemented in libopensc as well: sodium_pad
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, 3).as_slice(), &[0x80u8,0,0,0,0]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, 7).as_slice(), &[0x80u8]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, 0).as_slice(), &[0x80u8, 0,0,0,0,0,0,0]);

        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, 3).as_slice(), &[0x80u8,0,0,0,0]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, 7).as_slice(), &[0x80u8]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, 0).as_slice(), &[0u8; 0]);

        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_PKCS5, 3).as_slice(), &[0x05u8; 5]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_PKCS5, 7).as_slice(), &[0x01u8; 1]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_PKCS5, 0).as_slice(), &[0x08u8; 8]);

        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, 3).as_slice(), &[0u8,0,0,0,5]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, 7).as_slice(), &[1u8]);
        assert_eq!(trailing_blockcipher_padding_calculate(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, 0).as_slice(), &[0u8,0,0,0,0,0,0,8]);
    }
    #[test]
    fn test_trailing_blockcipher_padding_get_length() {
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ZEROES, &[0u8,2,1,0,0,0,0,0]).unwrap(), 5);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ZEROES, &[0u8,6,5,4,3,2,1,0]).unwrap(), 1);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ZEROES, &[0u8,7,6,5,4,3,2,1]).unwrap(), 0);

        // something similar is implemented in libopensc as well: sodium_unpad
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, &[0u8,0,0,0x80,0,0,0,0]).unwrap(), 5);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, &[0u8,0,0,0,0,0,0,0x80]).unwrap(), 1);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES, &[0x80u8,0,0,0,0,0,0,0]).unwrap(), 8);

        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, &[0u8,0,0,0x80,0,0,0,0]).unwrap(), 5);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, &[0u8,0,0,0,0,0,0,0x80]).unwrap(), 1);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ONEANDZEROES_ACOS5, &[0x80u8,0,0,0,0,0,0,0]).unwrap(), 0);

        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_PKCS5, &[0u8,5,5,5,5,5,5,5]).unwrap(), 5);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_PKCS5, &[0u8,1,1,1,1,1,1,1]).unwrap(), 1);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_PKCS5, &[8u8,8,8,8,8,8,8,8]).unwrap(), 8);

        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, &[0u8,0,0,0,0,0,0,5]).unwrap(), 5);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, &[0u8,0,0,0,0,0,0,1]).unwrap(), 1);
        assert_eq!(trailing_blockcipher_padding_get_length(8,BLOCKCIPHER_PAD_TYPE_ANSIX9_23, &[0u8,0,0,0,0,0,0,8]).unwrap(), 8);
    }
}
