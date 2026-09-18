// Rasterise an SVG with AppKit. Usage: swift render-svg.swift in.svg out.png size
import AppKit
let args = CommandLine.arguments
guard args.count == 4, let img = NSImage(contentsOfFile: args[1]), let px = Int(args[3]) else {
    FileHandle.standardError.write("usage: render-svg.swift in.svg out.png size\n".data(using: .utf8)!)
    exit(2)
}
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
img.draw(in: NSRect(x: 0, y: 0, width: px, height: px))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
