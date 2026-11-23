import re, sys
path, function = sys.argv[1], sys.argv[2]
lines = open(path).read().split('\n')
start = next(i for i, l in enumerate(lines) if re.match(rf'    fn {function}\b', l) or re.match(rf'    pub\(crate\) fn {function}\b', l))
match_line = next(i for i in range(start, len(lines)) if lines[i] == '        Ok(match self {')
end = next(i for i in range(match_line, len(lines)) if lines[i] == '        })')
arms = []
current = None
for i in range(match_line + 1, end):
    l = lines[i]
    if l.startswith('            Self::') or l.startswith('            _ =>') or (current is None):
        current = [l]; arms.append(current)
    else:
        current.append(l)
def snake(name):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()
helpers = []
dispatch = []
used = set()
for arm in arms:
    text = '\n'.join(arm)
    head_end = None
    depth = 0
    for j, ch in enumerate(text):
        if text.startswith('=>', j) and depth == 0:
            head_end = j; break
        if ch in '{([': depth += 1
        if ch in '})]': depth -= 1
    pattern = text[:head_end].strip()
    body = text[head_end + 2:].strip()
    if body.endswith(','): body = body[:-1]
    body_head = body.split('\n')[0]
    if pattern == '_' or ' if ' in pattern or len(body.split('\n')) <= 2 or (body.count('\n') <= 2 and 'self.' in body and body.strip('{} \n').count('\n') == 0):
        dispatch.extend(arm); continue
    name = 'eval_' + snake(re.match(r'Self::(\w+)', pattern).group(1))
    while name in used: name += '_'
    used.add(name)
    wildcard = re.sub(r'\{[^{}]*\}', '{ .. }', pattern)
    dispatch.append(f'            {wildcard} => return self.{name}(ctx, arena),')
    ctx = 'ctx' if re.search(r'\bctx\b', body) else '_ctx'
    arena = 'arena' if re.search(r'\barena\b', body) else '_arena'
    body_lines = body.split('\n')
    indented = '\n'.join(('    ' + b[8:] if b.startswith('        ') else b) if k else b for k, b in enumerate(body_lines))
    helpers.append(f'''    fn {name}(&self, {ctx}: EvalContext<'_>, {arena}: &mut Arena) -> Result<Scalar, CoreError> {{
        let {('(' + pattern + ')') if '|' in pattern else pattern} = self else {{
            return Err(CoreError::protocol("Expected {name[5:].replace('_', ' ')}"));
        }};
        Ok({indented})
    }}''')
func_end = next(i for i in range(end, len(lines)) if lines[i] == '    }')
new = lines[:match_line + 1] + dispatch + lines[end:func_end + 1] + [h for h in '\n'.join(helpers).split('\n')] + lines[func_end + 1:]
open(path, 'w').write('\n'.join(new))
print(len(helpers), 'helpers', function)
