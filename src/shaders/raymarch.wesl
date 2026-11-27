import package::gbuffer_sample::sample_gbuffer;

fn march(
    origin: ptr<function, vec2<f32>>,
    direction: vec2<f32>
) -> bool {
    let dir = normalize(direction);
    let initial = *origin;

    let cell = vec2<i32>(floor(initial));

    let step = vec2<i32>(
        select(-1, 1, dir.x >= 0.0),
        select(-1, 1, dir.y >= 0.0)
    );

    let delta = vec2<f32>(
        select(1e30, abs(1.0 / dir.x), dir.x != 0.0),
        select(1e30, abs(1.0 / dir.y), dir.y != 0.0)
    );

    let next_boundary = vec2<f32>(
        f32(cell.x) + select(0.0, 1.0, dir.x >= 0.0),
        f32(cell.y) + select(0.0, 1.0, dir.y >= 0.0)
    );

    var t_max = vec2<f32>(
        abs(next_boundary.x - initial.x) * delta.x,
        abs(next_boundary.y - initial.y) * delta.y
    );

    var current = cell;

    for (var i = 0u; i < 2048u; i++) {
        var t: f32;

        if (t_max.x < t_max.y) {
            t = t_max.x;
            t_max.x += delta.x;
            current.x += step.x;
        } else {
            t = t_max.y;
            t_max.y += delta.y;
            current.y += step.y;
        }

        *origin = initial + dir * t;

        let sample = sample_gbuffer(vec2<f32>(current) + vec2<f32>(0.5));

        if (sample.gb_type != 0u) {
            return true;
        }
    }

    return false;
}
