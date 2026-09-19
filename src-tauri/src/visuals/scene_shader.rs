//! Specialize the native shader before handing it to the driver. A held look
//! must not compile all 52 scenes (or both sides of every Tron collection blend).
use super::{VisualUniforms, PERFORMANCE_SHADER};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum SceneKey {
    Held(u32),
    Collection(u32, u32),
}

impl SceneKey {
    pub fn for_scene(id: f32, uniforms: &VisualUniforms) -> Self {
        let id = id.round() as u32;
        if id != 32 {
            return Self::Held(id.min(52));
        }
        let chapter = (uniforms.spatial[0] / 8.0).floor() as u32;
        let dimension = uniforms.chromatic[3].round() as u32;
        if (uniforms.spatial[0] / 8.0).fract() <= 0.8 {
            Self::Held(33 + variant(chapter, dimension))
        } else {
            Self::Collection(variant(chapter, dimension), variant(chapter + 1, dimension))
        }
    }

    pub fn upcoming(uniforms: &VisualUniforms) -> Self {
        let chapter = (uniforms.spatial[0] / 8.0).floor() as u32;
        let dimension = uniforms.chromatic[3].round() as u32;
        if (uniforms.spatial[0] / 8.0).fract() <= 0.8 {
            Self::Collection(variant(chapter, dimension), variant(chapter + 1, dimension))
        } else {
            Self::Held(33 + variant(chapter + 1, dimension))
        }
    }

    pub fn source(self) -> String {
        // These are checked-in WGSL sources, not user input. Remove comments
        // before finding balanced function bodies so braces in prose are inert.
        let mut source = PERFORMANCE_SHADER
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
            .collect::<Vec<_>>()
            .join("\n");
        let id = match self {
            Self::Held(id) => id,
            Self::Collection(..) => 32,
        };
        source = source.replace(
            "let primary_id = u32(round(params.style_a.x));",
            &format!("let primary_id = {id}u;"),
        );
        let family = if id < 26 {
            let dispatch = function(&source, "visual_family");
            let case = format!("case {id}u: {{ ");
            dispatch
                .split_once(&case)
                .unwrap()
                .1
                .split_once('}')
                .unwrap()
                .0
                .to_string()
        } else {
            "return vec3<f32>(0.0);".to_string()
        };
        replace_body(&mut source, "visual_family", &family);
        let look = function(&source, "tron_look").to_string();
        let scene = match self {
            Self::Held(id) if (33..=48).contains(&id) => {
                source.push_str(&specialize_look(&look, id - 33, "tron_selected"));
                "return tron_selected(uv, params.spatial.x);".to_string()
            }
            Self::Collection(from, to) => {
                source.push_str(&specialize_look(&look, from, "tron_outgoing"));
                source.push_str(&specialize_look(&look, to, "tron_incoming"));
                "let blend = smoothstep(0.8, 1.0, fract(params.spatial.x / 8.0));
                 let outgoing = tron_outgoing(uv, params.spatial.x);
                 if blend <= 0.0 { return outgoing; }
                 return mix(outgoing, tron_incoming(uv, params.spatial.x), blend);"
                    .to_string()
            }
            _ => "return vec3<f32>(0.0);".to_string(),
        };
        replace_body(&mut source, "tron_scene", &scene);
        // Remove unreachable functions before WGSL translation as well as DXC.
        // Keep the shared frame effects verbatim; driver constant folding only
        // needs to resolve the small, fixed scene dispatch in fs_main.
        retain_reachable(source)
    }
}

fn variant(index: u32, dimension: u32) -> u32 {
    const TWO_D: [u32; 9] = [4, 5, 7, 10, 11, 12, 13, 14, 15];
    const THREE_D: [u32; 7] = [0, 1, 2, 3, 6, 8, 9];
    match dimension {
        1 => TWO_D[index as usize % TWO_D.len()],
        2 => THREE_D[index as usize % THREE_D.len()],
        _ => index % 16,
    }
}

fn block_end(source: &str, opening: usize) -> usize {
    let mut depth = 0;
    for (offset, byte) in source.as_bytes()[opening..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return opening + offset + 1;
        }
    }
    panic!("unclosed checked-in shader block");
}

fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("fn {name}("))
        .expect("checked-in shader function");
    let opening = start + source[start..].find('{').unwrap();
    &source[start..block_end(source, opening)]
}

fn replace_body(source: &mut String, name: &str, body: &str) {
    let start = source.find(&format!("fn {name}(")).unwrap();
    let opening = start + source[start..].find('{').unwrap();
    source.replace_range(opening + 1..block_end(source, opening) - 1, body);
}

fn specialize_look(source: &str, look: u32, name: &str) -> String {
    let source = source
        .replace("fn tron_look(", &format!("\nfn {name}("))
        .replace("look: u32, ", "");
    let source = select_look_branches(&source, look);
    assert!(!source.contains("look =="), "unspecialized Tron branch");
    source
}

fn select_look_branches(source: &str, look: u32) -> String {
    let Some(start) = source.find("if look == ") else {
        return source.to_string();
    };
    let mut cursor = start;
    let mut selected = "";
    let mut matched = false;
    let end = loop {
        let opening = cursor + source[cursor..].find('{').unwrap();
        let condition = source[cursor + 3..opening].trim();
        let matches = condition.split(" || ").any(|clause| {
            clause
                .strip_prefix("look == ")
                .unwrap()
                .trim_end_matches('u')
                .parse::<u32>()
                .unwrap()
                == look
        });
        let end = block_end(source, opening);
        if matches && !matched {
            selected = &source[opening..end];
            matched = true;
        }
        let tail = &source[end..];
        let trimmed = tail.trim_start();
        if trimmed.starts_with("else if look == ") {
            cursor = end + tail.len() - trimmed.len() + 5;
            continue;
        }
        if trimmed.starts_with("else {") {
            let opening = end + tail.len() - trimmed.len() + 5;
            let end = block_end(source, opening);
            if !matched {
                selected = &source[opening..end];
            }
            break end;
        }
        break end;
    };
    format!(
        "{}{}{}",
        &source[..start],
        select_look_branches(selected, look),
        select_look_branches(&source[end..], look)
    )
}

fn retain_reachable(mut source: String) -> String {
    let functions: Vec<_> = source
        .match_indices("fn ")
        .map(|(start, _)| {
            let name_end = start + source[start..].find('(').unwrap();
            let name = source[start + 3..name_end].to_string();
            let opening = start + source[start..].find('{').unwrap();
            let end = block_end(&source, opening);
            let before = source[..start].trim_end();
            let start = if before.ends_with("@vertex") {
                before.len() - 7
            } else if before.ends_with("@fragment") {
                before.len() - 9
            } else {
                start
            };
            (name, start, end)
        })
        .collect();
    let mut used = std::collections::HashSet::from(["vs_main".to_string(), "fs_main".to_string()]);
    loop {
        let count = used.len();
        for (name, start, end) in &functions {
            if !used.contains(name) {
                continue;
            }
            for (callee, _, _) in &functions {
                if source[*start..*end].contains(&format!("{callee}(")) {
                    used.insert(callee.clone());
                }
            }
        }
        if count == used.len() {
            break;
        }
    }
    for (name, start, end) in functions.into_iter().rev() {
        if !used.contains(&name) {
            source.replace_range(start..end, "");
        }
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_native_specialization_is_valid_and_excludes_the_library_dispatch() {
        let keys = (0..=52)
            .filter(|id| *id != 32)
            .map(SceneKey::Held)
            .chain((0..3).flat_map(|dimension| {
                (0..16).map(move |i| {
                    SceneKey::Collection(variant(i, dimension), variant(i + 1, dimension))
                })
            }));
        for key in keys {
            let source = key.source();
            assert!(!source.contains("fn tron_look("));
            assert!(!source.contains("fn vs_lines("));
            assert!(!source.contains("switch id"));
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|error| panic!("{key:?}: {}", error.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|error| panic!("{key:?}: {error}"));
            assert!(
                source.len() < PERFORMANCE_SHADER.len() / 2,
                "{key:?}: {} bytes",
                source.len()
            );
        }
    }
}
