#!/usr/bin/env swift

import AppKit

private func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("ERROR: \(message)\n".utf8))
    exit(1)
}

guard CommandLine.arguments.count == 3 else {
    fail("Usage: render-iphone-device-frame.swift <input.png> <output.png>")
}

let inputURL = URL(fileURLWithPath: CommandLine.arguments[1])
let outputURL = URL(fileURLWithPath: CommandLine.arguments[2])

guard let screenshot = NSImage(contentsOf: inputURL) else {
    fail("Could not read \(inputURL.path)")
}

let screenshotSize = NSSize(width: 1_206, height: 2_622)
guard screenshot.size.width > 0, screenshot.size.height > 0 else {
    fail("Input image has invalid dimensions")
}

let canvasSize = NSSize(width: 1_400, height: 2_860)
let outerRect = NSRect(x: 50, y: 20, width: 1_300, height: 2_820)
let screenRect = NSRect(x: 97, y: 119, width: screenshotSize.width, height: screenshotSize.height)
let canvas = NSImage(size: canvasSize)

canvas.lockFocus()
guard let graphicsContext = NSGraphicsContext.current else {
    fail("Could not create graphics context")
}
graphicsContext.imageInterpolation = .high

NSColor.clear.setFill()
NSRect(origin: .zero, size: canvasSize).fill()

NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.55)
shadow.shadowBlurRadius = 30
shadow.shadowOffset = NSSize(width: 0, height: -8)
shadow.set()
NSColor(calibratedWhite: 0.015, alpha: 1).setFill()
NSBezierPath(roundedRect: outerRect, xRadius: 178, yRadius: 178).fill()
NSGraphicsContext.restoreGraphicsState()

let outerBorder = NSBezierPath(roundedRect: outerRect, xRadius: 178, yRadius: 178)
outerBorder.lineWidth = 7
NSColor(calibratedWhite: 0.28, alpha: 1).setStroke()
outerBorder.stroke()

let innerBorder = NSBezierPath(
    roundedRect: outerRect.insetBy(dx: 13, dy: 13),
    xRadius: 166,
    yRadius: 166
)
innerBorder.lineWidth = 3
NSColor(calibratedWhite: 0.11, alpha: 1).setStroke()
innerBorder.stroke()

NSGraphicsContext.saveGraphicsState()
NSBezierPath(roundedRect: screenRect, xRadius: 112, yRadius: 112).addClip()
screenshot.draw(
    in: screenRect,
    from: NSRect(origin: .zero, size: screenshot.size),
    operation: .sourceOver,
    fraction: 1
)
NSGraphicsContext.restoreGraphicsState()

let screenBorder = NSBezierPath(roundedRect: screenRect, xRadius: 112, yRadius: 112)
screenBorder.lineWidth = 4
NSColor.black.setStroke()
screenBorder.stroke()

let islandRect = NSRect(
    x: canvasSize.width / 2 - 190,
    y: screenRect.maxY - 112,
    width: 380,
    height: 86
)
NSColor.black.setFill()
NSBezierPath(roundedRect: islandRect, xRadius: 43, yRadius: 43).fill()

func drawSideButton(_ rect: NSRect) {
    let button = NSBezierPath(roundedRect: rect, xRadius: 8, yRadius: 8)
    NSColor(calibratedWhite: 0.055, alpha: 1).setFill()
    button.fill()
    button.lineWidth = 2
    NSColor(calibratedWhite: 0.35, alpha: 1).setStroke()
    button.stroke()
}

drawSideButton(NSRect(x: 37, y: 2_110, width: 17, height: 175))
drawSideButton(NSRect(x: 37, y: 1_820, width: 17, height: 230))
drawSideButton(NSRect(x: 37, y: 1_540, width: 17, height: 230))
drawSideButton(NSRect(x: 1_346, y: 1_820, width: 17, height: 390))

canvas.unlockFocus()

guard
    let tiff = canvas.tiffRepresentation,
    let representation = NSBitmapImageRep(data: tiff),
    let png = representation.representation(using: .png, properties: [:])
else {
    fail("Could not encode framed PNG")
}

do {
    try png.write(to: outputURL, options: .atomic)
} catch {
    fail("Could not write \(outputURL.path): \(error.localizedDescription)")
}

print("Rendered \(outputURL.path)")
