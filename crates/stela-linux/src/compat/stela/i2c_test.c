// SPDX-License-Identifier: GPL-2.0-or-later
/* Linux's I2C core on the fixture's real Runtime task, which its bus lock
 * needs: adapters, transfers with Linux's retries and within an adapter's
 * quirks, and unbound clients. */
#include <linux/err.h>
#include <linux/i2c.h>
#include <linux/jiffies.h>
#include <linux/string.h>
#include <stela/device.h>
#include <stela/test.h>

void stela_linux_i2c_test(void);

static int results[2];
static unsigned int transfers;

static int transfer(struct i2c_adapter *adapter, struct i2c_msg *msgs, int num)
{
	(void)adapter;
	(void)msgs;
	(void)num;
	return results[transfers++];
}

static u32 functionality(struct i2c_adapter *adapter)
{
	(void)adapter;
	return I2C_FUNC_I2C;
}

static const struct i2c_algorithm algorithm = {.xfer = transfer, .functionality = functionality};

static struct i2c_adapter adapter = {.algo = &algorithm, .retries = 1, .name = "test"};
static struct i2c_adapter managed = {.algo = &algorithm, .name = "managed"};

/* The quirks an RX 9070's SMU sets on its I2C controller
 * (smu_v14_0_2_i2c_control_quirks), with its 24-command limit. */
static const struct i2c_adapter_quirks smu_quirks = {
	.flags = I2C_AQ_COMB | I2C_AQ_COMB_SAME_ADDR | I2C_AQ_NO_ZERO_LEN,
	.max_read_len = 24,
	.max_write_len = 24,
	.max_comb_1st_msg_len = 2,
	.max_comb_2nd_msg_len = 22,
};
static struct i2c_adapter quirked = {.algo = &algorithm, .quirks = &smu_quirks, .name = "smu"};

static void parent_released(struct device *dev)
{
	(void)dev;
}

void stela_linux_i2c_test(void)
{
	assert(!i2c_add_adapter(&adapter) && adapter.timeout == HZ);
	assert(!strcmp(dev_name(&adapter.dev), "i2c-0"));

	/* Arbitration loss retries up to the limit; other errors do not. */
	results[0] = -EAGAIN;
	results[1] = 2;
	assert(i2c_transfer(&adapter, NULL, 2) == 2 && transfers == 2);
	transfers = 0;
	results[0] = results[1] = -EAGAIN;
	assert(i2c_transfer(&adapter, NULL, 1) == -EAGAIN && transfers == 2);
	transfers = 0;
	results[0] = -EIO;
	assert(i2c_transfer(&adapter, NULL, 1) == -EIO && transfers == 1);

	assert(i2c_trylock_bus(&adapter, I2C_LOCK_SEGMENT));
	assert(!i2c_trylock_bus(&adapter, I2C_LOCK_SEGMENT));
	i2c_unlock_bus(&adapter, I2C_LOCK_SEGMENT);

	struct i2c_board_info info = {I2C_BOARD_INFO("sensor", 0x48)};
	struct i2c_client *client = i2c_new_client_device(&adapter, &info);
	assert(!IS_ERR(client) && client->addr == 0x48 && !client->dev.driver);
	assert(!strcmp(client->name, "sensor") && client->adapter == &adapter);
	i2c_unregister_device(client);
	i2c_unregister_device(ERR_PTR(-ENODEV));
	info.addr = 0x80;
	assert(IS_ERR(i2c_new_client_device(&adapter, &info)));

	i2c_del_adapter(&adapter);

	/* A managed adapter goes away with its device's managed actions, which
	 * wait for its release. */
	struct device parent = {.release = parent_released};
	device_initialize(&parent);
	assert(!devm_i2c_add_adapter(&parent, &managed));
	assert(!strcmp(dev_name(&managed.dev), "i2c-1"));
	stela_linux_devres_release(&parent);
	put_device(&parent);

	/* An adapter with quirks is added, and a transfer they do not admit
	 * never reaches it. */
	assert(!i2c_add_adapter(&quirked));
	u8 offset[2] = {0}, data[24] = {0};
	struct i2c_msg read[2] = {
		{.addr = 0x50, .len = 2, .buf = offset},
		{.addr = 0x50, .flags = I2C_M_RD, .len = 22, .buf = data},
	};
	transfers = 0;
	results[0] = 2;
	assert(i2c_transfer(&quirked, read, 2) == 2 && transfers == 1);
	read[1].addr = 0x51;
	assert(i2c_transfer(&quirked, read, 2) == -EOPNOTSUPP);
	read[1].addr = 0x50;
	read[1].len = 23;
	assert(i2c_transfer(&quirked, read, 2) == -EOPNOTSUPP && transfers == 1);
	struct i2c_msg single = {.addr = 0x50, .flags = I2C_M_RD, .len = 24, .buf = data};
	transfers = 0;
	results[0] = 1;
	assert(i2c_transfer(&quirked, &single, 1) == 1 && transfers == 1);
	single.len = 25;
	assert(i2c_transfer(&quirked, &single, 1) == -EOPNOTSUPP);
	single.len = 0;
	assert(i2c_transfer(&quirked, &single, 1) == -EOPNOTSUPP);
	struct i2c_msg three[3] = {read[0], read[0], read[0]};
	assert(i2c_transfer(&quirked, three, 3) == -EOPNOTSUPP && transfers == 1);
	i2c_del_adapter(&quirked);
}
