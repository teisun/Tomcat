package com.tomcat.websocket;

import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.requeset.CustomizeTopicReq;
import com.tomcat.controller.response.ProfileResp;
import com.tomcat.service.UserProfileService;
import com.unfbx.chatgpt.entity.chat.Message;
import lombok.extern.slf4j.Slf4j;
import org.java_websocket.WebSocket;
import org.junit.Assert;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestMethodOrder;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.junit4.SpringRunner;

import java.net.URI;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.awaitility.Awaitility.await;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.websocket
 * @className: WebSocketTest
 * @author: tomcat
 * @description: ws 单元测试
 * @date: 2023/9/6 5:49 PM
 * @version: 1.0
 */


@Slf4j
@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class WebSocketTest {

    MyWebSocketClient myWebSocketClient;

    private static String uId;
    private static String chatId;

    /**
     * @description 测试websocket连接
     * @param :
     * @return void
     * @author tomcat
     * @date 2023/9/6 6:12 PM
     */
    @Order(1)
    @Test
    public void _1websocketClientConnectTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });

        Assert.assertEquals(WebSocket.READYSTATE.OPEN, myWebSocketClient.getReadyState());
        myWebSocketClient.close();
    }

    @Order(2)
    @Test
    public void _2chatInitTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT_INIT);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatInitResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.chatInitResp.getData());
        uId = myWebSocketClient.chatInitResp.getData();
        myWebSocketClient.close();
    }

    @Order(3)
    @Test
    public void _3curriculumPlanTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.PLAN);
        chatReq.setUid(uId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.planResp != null && myWebSocketClient.planResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.planResp.getData() != null);
        myWebSocketClient.close();
    }

    @Order(4)
    @Test
    public void _4startTopicTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.START_TOPIC);
        chatReq.setData("At the Restaurant");
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.startTopicResp != null && myWebSocketClient.startTopicResp.getData() != null;
        });
        chatId = myWebSocketClient.startTopicResp.getChatId();
        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.startTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });

        myWebSocketClient.close();
    }

    @Order(5)
    @Test
    public void _5chatTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT);
        chatReq.setData("Hello!Good to see you! I'm Tom!");
        chatReq.setChatId(chatId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(60, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatTopicResp != null && myWebSocketClient.chatTopicResp.getData() != null;
        });
        myWebSocketClient.chatTopicResp = null;

        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.chatTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });
        myWebSocketClient.confirmResp = null;

        myWebSocketClient.close();
    }

    @Order(6)
    @Test
    public void _6offlineMsgTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT);
        chatReq.setData("Hello!Good to see you! I'm Tom!");
        chatReq.setChatId(chatId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        Thread.sleep(200);
        log.info(" _6offlineMsgTest: 关闭ws链接");
        myWebSocketClient.close();
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.CLOSED);
        });
        Thread.sleep(1000 * 5);
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        log.info(" _6offlineMsgTest: 重连");
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        log.info(" _6offlineMsgTest: 重连成功");
        ChatReq chatReq1 = new ChatReq();
        chatReq1.setCommand(Command.OFFLINE_MSG);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq1));
        await().atMost(10, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatOffMsgResp != null && myWebSocketClient.chatOffMsgResp.getData() != null;
        });
        log.info(" _6offlineMsgTest offline msg: " + JSONUtil.toJsonStr(myWebSocketClient.chatOffMsgResp.getData()));
        Assert.assertNotNull(myWebSocketClient.chatOffMsgResp.getData() != null);
        myWebSocketClient.close();
    }


    @Autowired
    private UserProfileService userProfileService;

    @Value("${ai.prompt.aitutor}")
    private String promptAitutor;
    @Value("${ai.prompt.version}")
    private String promptVersion;

    @Value("${ai.prompt.curriculum.limiter}")
    private String promptCurriculumLimiter;

    private static String contextTopics = "{\"topics\":[{\"topic\":\"Shopping\",\"objective\":[\"Learn vocabulary and expressions used in shopping\",\"Practice asking for prices, comparing products, and expressing needs\"]},{\"topic\":\"Booking Hotel\",\"objective\":[\"Learn vocabulary and expressions used in hotel booking\",\"Practice asking about room types, the booking process, and expressing preferences\"]},{\"topic\":\"At the Restaurant\",\"objective\":[\"Learn vocabulary and expressions used in ordering at a restaurant\",\"Practice reading menus, placing orders, and expressing preferences\"]},{\"topic\":\"Daily Routines\",\"objective\":[\"Learn vocabulary and expressions used in daily routines\",\"Practice discussing daily schedules and activities\"]}]},\"usage\":{\"promptTokens\":2049,\"completionTokens\":183,\"totalTokens\":2232}}";
    private static String contextAssistantChat = "{\"topic\":\"At the Restaurant\",\"assistant_sentence\":\"Good evening! Welcome to our restaurant. How many people are in your party?\",\"translate\":\"晚上好！欢迎光临我们的餐厅。您一共有几位客人？\",\"tips\":[\"There are two of us.\",\"We have a party of six.\",\"I'm dining alone.\"],\"missions\":[{\"status\":0,\"text\":\"Ask the waiter for a menu\"},{\"status\":0,\"text\":\"Inquire about the daily specials\"},{\"status\":0,\"text\":\"Order a drink\"},{\"status\":0,\"text\":\"Ask for recommendations\"},{\"status\":0,\"text\":\"Order a main course\"}]},\"usage\":{\"promptTokens\":2371,\"completionTokens\":235,\"totalTokens\":2606},\"chatId\":\"fa54f222-157a-432a-9d42-af96bf6d1573\",\"msgId\":\"chatcmpl-7yGJ6TNs3CaQgnpovLEUB7WyozZrl\"}";
    @Order(7)
    @Test
    public void _7chatInitByContextTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        // 初始化
        List<Message> messages = new ArrayList<>();
        Message msg0 = Message.builder().content(promptAitutor).role(Message.Role.USER).build();
        messages.add(msg0);
        Message msg1 = Message.builder().content(promptVersion).role(Message.Role.ASSISTANT).build();
        messages.add(msg1);

        // 添加用户配置到上下文
        ProfileResp profile = userProfileService.getByUserId("2c8989ec-234a-493a-9d85-aa44fb045b5f");
        String commandConfig = Command.CONFIG + " " + JSONUtil.toJsonStr(profile.buildConfig());
        log.info(commandConfig);
        Message msg2 = Message.builder().content(commandConfig).role(Message.Role.USER).build();
        messages.add(msg2);

        // 生成topics
        Message msg3 = Message.builder().content(Command.PLAN + promptCurriculumLimiter).role(Message.Role.USER).build();
        messages.add(msg3);
        Message msg4 = Message.builder().content(contextTopics).role(Message.Role.ASSISTANT).build();
        messages.add(msg4);


        // 开始聊天
        Message msg5 = Message.builder().content("/START_TOPIC At the Restaurant").role(Message.Role.USER).build();
        messages.add(msg5);
        Message msg6 = Message.builder().content(contextAssistantChat).role(Message.Role.ASSISTANT).build();
        messages.add(msg6);
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT_INIT_BY_CONTEXT);
        chatReq.setData(JSONUtil.toJsonStr(messages));
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatInitByContextResp != null && myWebSocketClient.chatInitByContextResp.getChatId() != null;
        });
        log.info("myWebSocketClient.chatInitByContextResp.getChatId(): " + myWebSocketClient.chatInitByContextResp.getChatId());


        ChatReq chatReq1 = new ChatReq();
        chatReq1.setCommand(Command.CHAT);
        chatReq1.setData("Hello!Good to see you! I'm Tom!");
        chatReq1.setChatId(myWebSocketClient.chatInitByContextResp.getChatId());
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq1));
        await().atMost(90, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatTopicResp != null && myWebSocketClient.chatTopicResp.getData() != null;
        });

        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.chatTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });

        myWebSocketClient.close();
        myWebSocketClient.chatTopicResp = null;
        myWebSocketClient.confirmResp = null;
    }


    @Order(8)
    @Test
    public void _8customizeTopicTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });

        ChatReq initReq = new ChatReq();
        initReq.setCommand(Command.CHAT_INIT);
        myWebSocketClient.send(JSONUtil.toJsonStr(initReq));
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatInitResp.getData() != null;
        });


        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CUSTOMIZE_TOPIC);
        CustomizeTopicReq cReq = new CustomizeTopicReq();
        cReq.setTopic("租房");
        cReq.setUser_role("租客");
        cReq.setAssistant_role("房东");
        chatReq.setData(JSONUtil.toJsonStr(cReq));
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.cTopicResp != null && myWebSocketClient.cTopicResp.getData() != null;
        });
        chatId = myWebSocketClient.cTopicResp.getChatId();
        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.cTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });

        myWebSocketClient.close();
        myWebSocketClient.confirmResp = null;
    }


}
