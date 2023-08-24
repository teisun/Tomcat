package com.tomcat.websocket;

import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.SimpleChannelInboundHandler;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import io.netty.util.AttributeKey;
import lombok.extern.slf4j.Slf4j;

@ChannelHandler.Sharable
@Slf4j
public class WebSocketHandler extends SimpleChannelInboundHandler<TextWebSocketFrame> {

    /**
     * 一旦连接，第一个被执行
     */
    @Override
    public void handlerAdded(ChannelHandlerContext ctx) throws Exception {
        log.info("有新的客户端链接：[{}]", ctx.channel().id().asLongText());
        // 添加到channelGroup 通道组
        ChannelHandlerPool.getChannelGroup().add(ctx.channel());
    }

    /**
     * 读取数据
     */
    @Override
    protected void channelRead0(ChannelHandlerContext ctx, TextWebSocketFrame msg) throws Exception {
        log.info("服务器收到消息：{}", msg.text());
        // 获取用户ID,关联channel
        String text = msg.text();
        String uid = text.split(":")[0];
        String message = text.split(":")[1];
//        JSONObject jsonObject = JSONUtil.parseObj(msg.text());
        ChannelHandlerPool.getChannelMap().put(uid, ctx.channel());

        // 将用户ID作为自定义属性加入到channel中，方便随时channel中获取用户ID
        AttributeKey<String> key = AttributeKey.valueOf("userId");
        ctx.channel().attr(key).setIfAbsent(uid);

        // 回复消息
        ctx.channel().writeAndFlush(new TextWebSocketFrame("服务器消息 "+ uid+"："+message));

        // TODO
//        假设我们的即时通信系统中,当一个用户发送消息时,需要做以下处理:
//
//        验证消息内容合法性
//        持久化消息到数据库
//        推送消息到订阅用户的设备上
//        异步生成消息摘要用于搜索

//        如果直接在WebSocket的消息回调中顺序处理以上逻辑,会有以下问题:
//
//        客户端需要等待所有处理完成才能收到响应,延迟大
//        数据库和搜索服务的问题会直接导致发送失败
//        程序逻辑复杂,不易维护
//        这个时候我们可以做以下优化:
//
//        在WebSocket中只处理验证消息、写入消息队列这些必须的逻辑
//        消息进入队列后异步进行其他处理,不阻塞WebSocket线程
//        消息处理错误不会影响消息发送的响应
//        客户端可以快速收到发送确认,提升用户体验
//        解耦不同业务逻辑,变更时只需要修改消费消息的服务
//        通过这种方式,我们可以利用消息队列实现异步处理与解耦,优化即时通信系统的性能、稳定性和扩展性。
//
//        具体技术上可以使用RabbitMQ、Kafka等成熟的消息队列,也可以自己实现一个简单的队列。
    }

    @Override
    public void handlerRemoved(ChannelHandlerContext ctx) throws Exception {
        log.info("用户下线了:{}", ctx.channel().id().asLongText());
        // 删除通道
        ChannelHandlerPool.getChannelGroup().remove(ctx.channel());
        removeUserId(ctx);
    }

    @Override
    public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) throws Exception {
        log.info("异常：{}", cause.getMessage());
        // 删除通道
        ChannelHandlerPool.getChannelGroup().remove(ctx.channel());
        removeUserId(ctx);
        ctx.close();
    }

    /**
     * 删除用户与channel的对应关系
     */
    private void removeUserId(ChannelHandlerContext ctx) {
        AttributeKey<String> key = AttributeKey.valueOf("userId");
        String userId = ctx.channel().attr(key).get();
        ChannelHandlerPool.getChannelMap().remove(userId);

    }
}

