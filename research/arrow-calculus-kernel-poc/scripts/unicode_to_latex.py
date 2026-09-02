#!/usr/bin/env python3
"""Replace Unicode characters with LaTeX commands, skipping YAML front matter."""
import sys

def replace_unicode(text):
    replacements = {
        'λ': r'$\lambda$',
        '∧': r'$\land$',
        '∨': r'$\lor$',
        '⊤': r'$\top$',
        '⊥': r'$\bot$',
        '×': r'$\times$',
        '→': r'$\to$',
        '⇝': r'$\leadsto$',
        '⟹': r'$\Longrightarrow$',
        '⟺': r'$\Longleftrightarrow$',
        '⇔': r'$\Leftrightarrow$',
        '⊢': r'$\vdash$',
        '⊩': r'$\Vdash$',
        '∀': r'$\forall$',
        '∃': r'$\exists$',
        '∈': r'$\in$',
        '∉': r'$\notin$',
        '≤': r'$\le$',
        '≥': r'$\ge$',
        '≠': r'$\ne$',
        '⊔': r'$\sqcup$',
        'Φ': r'$\Phi$',
        'φ': r'$\phi$',
        'Γ': r'$\Gamma$',
        'Δ': r'$\Delta$',
        'Σ': r'$\Sigma$',
        'Ω': r'$\Omega$',
        'α': r'$\alpha$',
        'β': r'$\beta$',
        'γ': r'$\gamma$',
        'δ': r'$\delta$',
        'σ': r'$\sigma$',
        'ω': r'$\omega$',
        'ψ': r'$\psi$',
        'θ': r'$\theta$',
        'ξ': r'$\xi$',
        'π': r'$\pi$',
        'ρ': r'$\rho$',
        'τ': r'$\tau$',
        'κ': r'$\kappa$',
        'μ': r'$\mu$',
        'ν': r'$\nu$',
        '–': '--',
        '—': '---',
        '’': "'",
        '‘': "'",
        '"': '"',
        '"': '"',
        '…': '...',
        '•': '-',
        '✓': 'OK',
        '✗': 'X',
    }
    for u, l in replacements.items():
        text = text.replace(u, l)
    return text

lines = sys.stdin.read().split('\n')
# Find YAML front matter
in_yaml = False
yaml_end = -1
if lines[0].strip() == '---':
    in_yaml = True
    for i, line in enumerate(lines[1:], 1):
        if line.strip() == '---':
            yaml_end = i
            break

if in_yaml and yaml_end >= 0:
    # Output YAML as-is, replace rest
    print('\n'.join(lines[:yaml_end+1]))
    print('\n'.join(replace_unicode('\n'.join(lines[yaml_end+1:]))))
else:
    print(replace_unicode('\n'.join(lines)))
