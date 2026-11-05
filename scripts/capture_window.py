# -*- coding: utf-8 -*-
# @Time    : 2026/11/4 22:50
# @Author  : fzf
# @FileName: capture_window.py
# @Software: Siskin

"""Capture an X11 window's compositor pixmap, even when another app overlaps.

Requires Python 3, Pillow, libX11 and libXcomposite. Takes a numeric window ID
and an output PNG path; does not activate, move or capture other windows.
"""

import argparse
import ctypes as c

from PIL import Image


class XImage(c.Structure):
    _fields_ = [
        ("width", c.c_int), ("height", c.c_int), ("xoffset", c.c_int),
        ("format", c.c_int), ("data", c.c_void_p), ("byte_order", c.c_int),
        ("bitmap_unit", c.c_int), ("bitmap_bit_order", c.c_int),
        ("bitmap_pad", c.c_int), ("depth", c.c_int), ("bytes_per_line", c.c_int),
        ("bits_per_pixel", c.c_int), ("red_mask", c.c_ulong),
        ("green_mask", c.c_ulong), ("blue_mask", c.c_ulong),
    ]


def capture(window_id, output):
    x11 = c.CDLL("libX11.so.6")
    composite = c.CDLL("libXcomposite.so.1")
    x11.XOpenDisplay.argtypes, x11.XOpenDisplay.restype = [c.c_char_p], c.c_void_p
    x11.XCloseDisplay.argtypes = [c.c_void_p]
    composite.XCompositeNameWindowPixmap.argtypes = [c.c_void_p, c.c_ulong]
    composite.XCompositeNameWindowPixmap.restype = c.c_ulong
    composite.XCompositeRedirectWindow.argtypes = [c.c_void_p, c.c_ulong, c.c_int]
    composite.XCompositeUnredirectWindow.argtypes = [c.c_void_p, c.c_ulong, c.c_int]
    x11.XSync.argtypes = [c.c_void_p, c.c_int]
    x11.XGetImage.argtypes = [
        c.c_void_p, c.c_ulong, c.c_int, c.c_int, c.c_uint, c.c_uint,
        c.c_ulong, c.c_int,
    ]
    x11.XGetImage.restype = c.POINTER(XImage)
    x11.XGetGeometry.argtypes = [
        c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_int),
        c.POINTER(c.c_int), c.POINTER(c.c_uint), c.POINTER(c.c_uint),
        c.POINTER(c.c_uint), c.POINTER(c.c_uint),
    ]
    x11.XFreePixmap.argtypes = [c.c_void_p, c.c_ulong]
    x11.XDestroyImage.argtypes = [c.POINTER(XImage)]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("No X11 display")
    pixmap = 0
    image = None
    try:
        composite.XCompositeRedirectWindow(display, window_id, 0)
        x11.XSync(display, 0)
        pixmap = composite.XCompositeNameWindowPixmap(display, window_id)
        root, x, y = c.c_ulong(), c.c_int(), c.c_int()
        width, height, border, depth = (c.c_uint() for _ in range(4))
        if not x11.XGetGeometry(
            display, pixmap, c.byref(root), c.byref(x), c.byref(y),
            c.byref(width), c.byref(height), c.byref(border), c.byref(depth),
        ):
            raise RuntimeError("Cannot read window pixmap geometry")
        image = x11.XGetImage(
            display, pixmap, 0, 0, width.value, height.value, c.c_ulong(-1), 2,
        )
        if not image:
            raise RuntimeError("Cannot read window pixmap")
        data = image.contents
        if data.bits_per_pixel != 32 or data.byte_order != 0:
            raise RuntimeError(
                "Unsupported pixel format: bpp={}, order={}, masks={:x}/{:x}/{:x}".format(
                    data.bits_per_pixel, data.byte_order,
                    data.red_mask, data.green_mask, data.blue_mask,
                )
            )
        raw = c.string_at(data.data, data.bytes_per_line * data.height)
        Image.frombytes(
            "RGB", (data.width, data.height), raw, "raw", "BGRX", data.bytes_per_line,
        ).save(output)
    finally:
        if image:
            x11.XDestroyImage(image)
        if pixmap:
            x11.XFreePixmap(display, pixmap)
        composite.XCompositeUnredirectWindow(display, window_id, 0)
        x11.XCloseDisplay(display)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("window", type=lambda value: int(value, 0))
    parser.add_argument("output")
    args = parser.parse_args()
    capture(args.window, args.output)
