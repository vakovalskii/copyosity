#!/usr/bin/env swift
import CoreGraphics
import Foundation

let expectedLayer = 24

guard let info = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]]
else {
    fputs("failed to read window list\n", stderr)
    exit(1)
}

var hits = 0
for entry in info {
    guard let owner = entry[kCGWindowOwnerName as String] as? String,
          owner.localizedCaseInsensitiveContains("copyosity") else { continue }

    let title = entry[kCGWindowName as String] as? String ?? ""
    let layer = entry[kCGWindowLayer as String] as? Int ?? -1
    let bounds = entry[kCGWindowBounds as String] as? [String: CGFloat] ?? [:]
    let height = bounds["Height"] ?? 0
    if height < 40 { continue }

    print("copyosity window: title='\(title)' layer=\(layer) height=\(Int(height))")
    if layer == expectedLayer {
        hits += 1
    }
}

if hits == 0 {
    fputs("no Copyosity windows at auxiliary level \(expectedLayer)\n", stderr)
    exit(2)
}

print("ok: \(hits) Copyosity window(s) at level \(expectedLayer)")
