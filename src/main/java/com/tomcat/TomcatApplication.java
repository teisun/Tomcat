package com.tomcat;

import com.tomcat.config.TaskConfig;
import com.tomcat.websocket.NettyServer;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.ApplicationArguments;
import org.springframework.boot.ApplicationRunner;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.Import;
import org.springframework.core.task.TaskExecutor;

import java.util.List;

@SpringBootApplication
@Import(TaskConfig.class)
public class TomcatApplication {

    @Value("${chatgpt.apiKey}")
    private List<String> apiKey;
    @Value("${chatgpt.apiHost}")
    private String apiHost;


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


}