package com.tomcat;

import com.tomcat.config.TaskConfig;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Import;

@SpringBootApplication
@Import(TaskConfig.class)
public class TomcatApplication {




    public static void main(String[] args) {
        System.out.println("Hello Tomcat!");
        SpringApplication.run(TomcatApplication.class, args);

    }


    //TODO
    // 1. done 支持存储中文
    // 2. done findByDeviceId改造
    // 3. done 修改userId
    // 4. done 服务端错误处理
    // 5. done 单元测试规范化
    // 6. .. 密钥改造
    // 7. done 整理websocket的配置项
    // websocket 单元测试
    // 处理websocket断线重连逻辑
    // 处理Ai的聊天返回
    // 异常指令处理
    // prompt添加用户配置
    // 离线消息


}