"""Draw storyboard primitives at double density in logical coordinates."""
from PIL import Image, ImageDraw


class Storyboard:
    scale = 2

    def __init__(self, width, height):
        self.image = Image.new("RGB", (width * self.scale, height * self.scale), "black")
        self.draw = ImageDraw.Draw(self.image)

    def coordinates(self, values):
        return tuple(self.coordinates(v) if isinstance(v, (tuple, list)) else
                     round(v * self.scale) for v in values)

    def text(self, position, value, **style):
        self.draw.text(self.coordinates(position), value, **style)

    def line(self, points, width=1, **style):
        self.draw.line(self.coordinates(points), width=width * self.scale, **style)

    def polygon(self, points, **style):
        self.draw.polygon(self.coordinates(points), **style)

    def ellipse(self, bounds, **style):
        self.draw.ellipse(self.coordinates(bounds), **style)

    def rounded_rectangle(self, bounds, radius, width=1, **style):
        self.draw.rounded_rectangle(self.coordinates(bounds), radius * self.scale,
                                    width=width * self.scale, **style)
