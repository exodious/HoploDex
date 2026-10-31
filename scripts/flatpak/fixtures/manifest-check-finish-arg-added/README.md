manifest-check-ok with `--share=network` added to finish-args: the check
manifest must not widen the shipped permissions (the run adds the network at
`flatpak run`, 011 research.md §16). The comparison must fail naming finish-args
(011 FR-021).
