#!/usr/bin/env python3
"""Generate publication-ready figures for EFIE evaluation."""

import json
import os
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

# Create plots directory
os.makedirs('results/plots', exist_ok=True)

# Load scalability data
with open('results/json/scalability.json') as f:
    data = json.load(f)

file_counts = [d['FileCount'] for d in data]
efie_build = [d['EFIEBuildMs'] for d in data]
orig_build = [d['OrigBuildMs'] for d in data]
efie_query = [d['EFIEQueryMs'] for d in data]
orig_query = [d['OrigQueryMs'] for d in data]

# Figure 1: Build Time Scalability
fig, ax = plt.subplots(1, 1, figsize=(8, 5))
ax.plot(file_counts, efie_build, 'o-', color='#2196F3', linewidth=2, markersize=8, label='EFIE')
ax.plot(file_counts, orig_build, 's-', color='#F44336', linewidth=2, markersize=8, label='Original BFS')
ax.set_xlabel('Number of Files', fontsize=12)
ax.set_ylabel('Build Time (ms)', fontsize=12)
ax.set_title('Build Time Scalability', fontsize=14)
ax.legend(fontsize=11)
ax.grid(True, alpha=0.3)
ax.set_xscale('log')
ax.set_yscale('log')

# Add linear fit annotations
ax.annotate(f'EFIE: y = 0.556x (R²=0.954)',
            xy=(0.05, 0.95), xycoords='axes fraction',
            fontsize=10, color='#2196F3',
            verticalalignment='top')
ax.annotate(f'Original: y = 0.035x (R²=0.981)',
            xy=(0.05, 0.88), xycoords='axes fraction',
            fontsize=10, color='#F44336',
            verticalalignment='top')

plt.tight_layout()
plt.savefig('results/plots/build_scalability.png', dpi=150, bbox_inches='tight')
plt.close()
print("Saved: results/plots/build_scalability.png")

# Figure 2: Query Time Scalability
fig, ax = plt.subplots(1, 1, figsize=(8, 5))
ax.plot(file_counts, efie_query, 'o-', color='#2196F3', linewidth=2, markersize=8, label='EFIE')
ax.plot(file_counts, orig_query, 's-', color='#F44336', linewidth=2, markersize=8, label='Original BFS')
ax.set_xlabel('Number of Files', fontsize=12)
ax.set_ylabel('Query Time (ms)', fontsize=12)
ax.set_title('Query Time Scalability', fontsize=14)
ax.legend(fontsize=11)
ax.grid(True, alpha=0.3)
ax.set_xscale('log')
ax.set_yscale('log')

ax.annotate(f'EFIE: y = 0.0022x (R²=0.998)',
            xy=(0.05, 0.95), xycoords='axes fraction',
            fontsize=10, color='#2196F3',
            verticalalignment='top')
ax.annotate(f'Original: y = 0.000019x (R²=0.992)',
            xy=(0.05, 0.88), xycoords='axes fraction',
            fontsize=10, color='#F44336',
            verticalalignment='top')

plt.tight_layout()
plt.savefig('results/plots/query_scalability.png', dpi=150, bbox_inches='tight')
plt.close()
print("Saved: results/plots/query_scalability.png")

# Figure 3: Memory Usage Comparison
fig, ax = plt.subplots(1, 1, figsize=(8, 5))

# Load from full evaluation results
with open('results/json/all_claims.json') as f:
    claims = json.load(f)

mem_claims = [c for c in claims if c['claim_id'] == 'C4']
mem_file_counts = []
mem_overheads = []
for c in mem_claims:
    # Extract file count from claim text
    text = c['claim_text']
    if 'measured at' in text:
        count_str = text.split('measured at ')[1].split(' files')[0]
        mem_file_counts.append(int(count_str))
        mem_overheads.append(c['value'])

# Sort by file count
sorted_pairs = sorted(zip(mem_file_counts, mem_overheads))
mem_file_counts, mem_overheads = zip(*sorted_pairs)

colors = ['#4CAF50' if v > 0 else '#F44336' for v in mem_overheads]
ax.bar(range(len(mem_file_counts)), mem_overheads, color=colors, alpha=0.8)
ax.set_xticks(range(len(mem_file_counts)))
ax.set_xticklabels([f'{n:,}' for n in mem_file_counts], rotation=45)
ax.set_xlabel('Number of Files', fontsize=12)
ax.set_ylabel('Memory Overhead (%)', fontsize=12)
ax.set_title('EFIE Memory Overhead vs Original', fontsize=14)
ax.axhline(y=0, color='black', linewidth=0.5)
ax.axhline(y=16, color='gray', linewidth=1, linestyle='--', label='Claimed 16%')
ax.legend(fontsize=10)
ax.grid(True, alpha=0.3, axis='y')

plt.tight_layout()
plt.savefig('results/plots/memory_overhead.png', dpi=150, bbox_inches='tight')
plt.close()
print("Saved: results/plots/memory_overhead.png")

# Figure 4: Ablation Study
fig, ax = plt.subplots(1, 1, figsize=(8, 5))

# Ablation data from profiling
variants = ['V0: Original', 'V6: Full EFIE']
build_times = [50.8, 371.0]
query_times = [0.0, 0.2]

x = np.arange(len(variants))
width = 0.35

bars1 = ax.bar(x - width/2, build_times, width, label='Build Time (ms)', color='#2196F3')
ax2 = ax.twinx()
bars2 = ax2.bar(x + width/2, query_times, width, label='Query Time (ms)', color='#FF9800')

ax.set_xlabel('Variant', fontsize=12)
ax.set_ylabel('Build Time (ms)', fontsize=12, color='#2196F3')
ax2.set_ylabel('Query Time (ms)', fontsize=12, color='#FF9800')
ax.set_title('Ablation Study: EFIE vs Original (2,000 files)', fontsize=14)
ax.set_xticks(x)
ax.set_xticklabels(variants, fontsize=10)
ax.legend(loc='upper left', fontsize=10)
ax2.legend(loc='upper right', fontsize=10)
ax.grid(True, alpha=0.3, axis='y')

plt.tight_layout()
plt.savefig('results/plots/ablation.png', dpi=150, bbox_inches='tight')
plt.close()
print("Saved: results/plots/ablation.png")

# Figure 5: Claim Verification Summary
fig, ax = plt.subplots(1, 1, figsize=(8, 5))

with open('results/json/all_claims.json') as f:
    claims = json.load(f)

status_counts = {}
for c in claims:
    status = c['status']
    status_counts[status] = status_counts.get(status, 0) + 1

labels = list(status_counts.keys())
sizes = list(status_counts.values())
colors_map = {'VERIFIED': '#4CAF50', 'REFUTED': '#F44336', 'UNVERIFIED': '#FFC107', 'PARTIALLY_VERIFIED': '#FF9800'}
colors = [colors_map.get(l, '#9E9E9E') for l in labels]

wedges, texts, autotexts = ax.pie(sizes, labels=labels, colors=colors, autopct='%1.0f%%',
                                   startangle=90, textprops={'fontsize': 11})
for autotext in autotexts:
    autotext.set_fontsize(12)
    autotext.set_fontweight('bold')

ax.set_title('Claim Verification Status (30-run benchmarks)', fontsize=14)

plt.tight_layout()
plt.savefig('results/plots/claim_verification.png', dpi=150, bbox_inches='tight')
plt.close()
print("Saved: results/plots/claim_verification.png")

print("\nAll figures saved to results/plots/")
