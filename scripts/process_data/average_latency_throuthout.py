import pandas as pd
import os

# Define column names
column_names = ['epoch_id', 'validator num', 'contributor num', 'batch num', 'tx size(Bytes)/tx', 'block num', 'epoch_time(latency)']

current_dir = os.getcwd()
parent_dir = os.path.dirname(current_dir)

# Read the markdown file
directory_path = parent_dir + "/test-data/test_data_7_30_1600_20_4_delay100/hbb-node1"

data_frames = []
for filename in os.listdir(directory_path):
    if filename.endswith(".md"):  # Only process .md files
        with open(os.path.join(directory_path, filename), 'r') as f:
            data = f.read()

        # Split the table into lines
        lines = data.split('\n')[3:-1]  # Exclude the header and the alignment line and the last empty line

        # Split each line into its components
        table_data = [line.split('|')[1:-1] for line in lines]  # Exclude the first and last empty strings

        # Convert the table data into a pandas DataFrame and append it to the list
        df = pd.DataFrame(table_data, columns=column_names)

        # Convert numeric columns to the correct data type
        for col in ['validator num', 'block num', 'epoch_time(latency)']:
            df[col] = pd.to_numeric(df[col])

        data_frames.append(df)

# Concatenate all data frames
df = pd.concat(data_frames)

grouped = df.groupby('validator num').agg({'block num': 'mean', 'epoch_time(latency)': 'mean'})

import matplotlib.pyplot as plt
import os

# 创建图形窗口和子图
fig, axes = plt.subplots(2, 1, figsize=(10, 10))

# 绘制平均吞吐量图像
axes[0].plot(grouped.index, grouped['block num'])
axes[0].set_xlabel('Number of Nodes')
axes[0].set_ylabel('Average Throughput')
axes[0].set_title('Average Throughput vs Number of Nodes')

# 绘制平均延迟图像
axes[1].plot(grouped.index, grouped['epoch_time(latency)'])
axes[1].set_xlabel('Number of Nodes')
axes[1].set_ylabel('Average Latency')
axes[1].set_title('Average Latency vs Number of Nodes')

# 调整子图间的间距
plt.tight_layout()

save_dir = parent_dir + '/analysis-fig'
os.makedirs(save_dir, exist_ok=True)  # 使用os.makedirs()创建目录，exist_ok=True表示如果目录已存在则不会引发异常

# 保存图像
save_path = os.path.join(save_dir, 'dataloading.png')
plt.savefig(save_path)

# Show the plots
# plt.show()