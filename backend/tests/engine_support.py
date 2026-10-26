import json

import pytest


class Core:
    def __init__(self):
        import fvx_core

        self.module = fvx_core
        self.engine = fvx_core.Engine()

    def run(self, command, **params):
        return json.loads(self.engine.execute(command, json.dumps(params)))

    def fails(self, command, **params):
        with pytest.raises(ValueError) as caught:
            self.engine.execute(command, json.dumps(params))
        return str(caught.value)

    def comp(self, name="Test", width=64, height=48, fps=10, duration=2.0, background=None):
        params = {"name": name, "width": width, "height": height, "frameRate": fps, "duration": duration}
        if background is not None:
            params["background"] = background
        return self.run("comp.new", **params)["comp"]

    def solid(self, name="Solid", color="#ff0000", **extra):
        return self.run("layer.newSolid", name=name, color=color, **extra)

    def prop(self, layer, path, time=None):
        params = {"layer": layer, "path": path}
        if time is not None:
            params["time"] = time
        return self.run("prop.get", **params)

    def value(self, layer, path, time):
        return self.prop(layer, path, time)["value"]

    def frame(self, time=0.0, comp=None):
        params = {"time": time, "display": False, "guides": False, "cache": False}
        if comp is not None:
            params["comp"] = comp
        width, height, pixels = self.engine.frame_job(json.dumps(params)).run()
        return width, height, bytes(pixels)
